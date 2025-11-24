use core::time::Duration as UnsignedDuration;

use crate::{
    civil::{DateTime, Era, ISOWeekDate, Time, Weekday},
    duration::{Duration, SDuration},
    error::{err, Error, ErrorContext},
    fmt::{
        self,
        temporal::{DEFAULT_DATETIME_PARSER, DEFAULT_DATETIME_PRINTER},
    },
    shared::util::itime::{self, IDate, IEpochDay},
    tz::TimeZone,
    util::{
        rangeint::{self, Composite, RFrom, RInto, TryRFrom},
        t::{self, Day, Month, Sign, UnixEpochDay, Year, C},
    },
    RoundMode, SignedDuration, Span, SpanRound, Unit, Zoned,
};

/// A representation of a civil date in the Gregorian calendar.
///
/// A `Date` value corresponds to a triple of year, month and day. Every `Date`
/// value is guaranteed to be a valid Gregorian calendar date. For example,
/// both `2023-02-29` and `2023-11-31` are invalid and cannot be represented by
/// a `Date`.
///
/// # Civil dates
///
/// A `Date` value behaves without regard to daylight saving time or time
/// zones in general. When doing arithmetic on dates with spans defined in
/// units of time (such as with [`Date::checked_add`]), days are considered to
/// always be precisely `86,400` seconds long.
///
/// # Parsing and printing
///
/// The `Date` type provides convenient trait implementations of
/// [`std::str::FromStr`] and [`std::fmt::Display`]:
///
/// ```
/// use jiff::civil::Date;
///
/// let date: Date = "2024-06-19".parse()?;
/// assert_eq!(date.to_string(), "2024-06-19");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A civil `Date` can also be parsed from something that _contains_ a date,
/// but with perhaps other data (such as an offset or time zone):
///
/// ```
/// use jiff::civil::Date;
///
/// let date: Date = "2024-06-19T15:22:45-04[America/New_York]".parse()?;
/// assert_eq!(date.to_string(), "2024-06-19");
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
/// value corresponds to `0000-01-01`. One can also access this value via the
/// `Date::ZERO` constant.
///
/// # Comparisons
///
/// The `Date` type provides both `Eq` and `Ord` trait implementations to
/// facilitate easy comparisons. When a date `d1` occurs before a date `d2`,
/// then `d1 < d2`. For example:
///
/// ```
/// use jiff::civil::date;
///
/// let d1 = date(2024, 3, 11);
/// let d2 = date(2025, 1, 31);
/// assert!(d1 < d2);
/// ```
///
/// # Arithmetic
///
/// This type provides routines for adding and subtracting spans of time, as
/// well as computing the span of time between two `Date` values.
///
/// For adding or subtracting spans of time, one can use any of the following
/// routines:
///
/// * [`Date::checked_add`] or [`Date::checked_sub`] for checked arithmetic.
/// * [`Date::saturating_add`] or [`Date::saturating_sub`] for saturating
/// arithmetic.
///
/// Additionally, checked arithmetic is available via the `Add` and `Sub`
/// trait implementations. When the result overflows, a panic occurs.
///
/// ```
/// use jiff::{civil::date, ToSpan};
///
/// let start = date(2024, 2, 25);
/// let one_week_later = start + 1.weeks();
/// assert_eq!(one_week_later, date(2024, 3, 3));
/// ```
///
/// One can compute the span of time between two dates using either
/// [`Date::until`] or [`Date::since`]. It's also possible to subtract two
/// `Date` values directly via a `Sub` trait implementation:
///
/// ```
/// use jiff::{civil::date, ToSpan};
///
/// let date1 = date(2024, 3, 3);
/// let date2 = date(2024, 2, 25);
/// assert_eq!(date1 - date2, 7.days().fieldwise());
/// ```
///
/// The `until` and `since` APIs are polymorphic and allow re-balancing and
/// rounding the span returned. For example, the default largest unit is days
/// (as exemplified above), but we can ask for bigger units:
///
/// ```
/// use jiff::{civil::date, ToSpan, Unit};
///
/// let date1 = date(2024, 5, 3);
/// let date2 = date(2024, 2, 25);
/// assert_eq!(
///     date1.since((Unit::Year, date2))?,
///     2.months().days(7).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Or even round the span returned:
///
/// ```
/// use jiff::{civil::{DateDifference, date}, RoundMode, ToSpan, Unit};
///
/// let date1 = date(2024, 5, 15);
/// let date2 = date(2024, 2, 25);
/// assert_eq!(
///     date1.since(
///         DateDifference::new(date2)
///             .smallest(Unit::Month)
///             .largest(Unit::Year),
///     )?,
///     2.months().fieldwise(),
/// );
/// // `DateDifference` uses truncation as a rounding mode by default,
/// // but you can set the rounding mode to break ties away from zero:
/// assert_eq!(
///     date1.since(
///         DateDifference::new(date2)
///             .smallest(Unit::Month)
///             .largest(Unit::Year)
///             .mode(RoundMode::HalfExpand),
///     )?,
///     // Rounds up to 8 days.
///     3.months().fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Rounding
///
/// Rounding dates is currently not supported. If you want this functionality,
/// please participate in the [issue tracking its support][add-date-rounding].
///
/// [add-date-rounding]: https://github.com/BurntSushi/jiff/issues/1
#[derive(Clone, Copy, Hash)]
pub struct Date {
    year: Year,
    month: Month,
    day: Day,
}

impl Date {
    /// The minimum representable Gregorian date.
    ///
    /// The minimum is chosen such that any [`Timestamp`](crate::Timestamp)
    /// combined with any valid time zone offset can be infallibly converted to
    /// this type. This means that the minimum `Timestamp` is guaranteed to be
    /// bigger than the minimum `Date`.
    pub const MIN: Date = Date::constant(-9999, 1, 1);

    /// The maximum representable Gregorian date.
    ///
    /// The maximum is chosen such that any [`Timestamp`](crate::Timestamp)
    /// combined with any valid time zone offset can be infallibly converted to
    /// this type. This means that the maximum `Timestamp` is guaranteed to be
    /// smaller than the maximum `Date`.
    pub const MAX: Date = Date::constant(9999, 12, 31);

    /// The first day of the zeroth year.
    ///
    /// This is guaranteed to be equivalent to `Date::default()`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::Date;
    ///
    /// assert_eq!(Date::ZERO, Date::default());
    /// ```
    pub const ZERO: Date = Date::constant(0, 1, 1);

    /// Creates a new `Date` value from its component year, month and day
    /// values.
    ///
    /// To set the component values of a date after creating it, use
    /// [`DateWith`] via [`Date::with`] to build a new [`Date`] from the fields
    /// of an existing date.
    ///
    /// # Errors
    ///
    /// This returns an error when the given year-month-day does not
    /// correspond to a valid date. Namely, all of the following must be
    /// true:
    ///
    /// * The year must be in the range `-9999..=9999`.
    /// * The month must be in the range `1..=12`.
    /// * The day must be at least `1` and must be at most the number of days
    /// in the corresponding month. So for example, `2024-02-29` is valid but
    /// `2023-02-29` is not.
    ///
    /// # Example
    ///
    /// This shows an example of a valid date:
    ///
    /// ```
    /// use jiff::civil::Date;
    ///
    /// let d = Date::new(2024, 2, 29).unwrap();
    /// assert_eq!(d.year(), 2024);
    /// assert_eq!(d.month(), 2);
    /// assert_eq!(d.day(), 29);
    /// ```
    ///
    /// This shows an example of an invalid date:
    ///
    /// ```
    /// use jiff::civil::Date;
    ///
    /// assert!(Date::new(2023, 2, 29).is_err());
    /// ```
    #[inline]
    pub fn new(year: i16, month: i8, day: i8) -> Result<Date, Error> {
        let year = Year::try_new("year", year)?;
        let month = Month::try_new("month", month)?;
        let day = Day::try_new("day", day)?;
        Date::new_ranged(year, month, day)
    }

    /// Creates a new `Date` value in a `const` context.
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
    /// # Example
    ///
    /// ```
    /// use jiff::civil::Date;
    ///
    /// let d = Date::constant(2024, 2, 29);
    /// assert_eq!(d.year(), 2024);
    /// assert_eq!(d.month(), 2);
    /// assert_eq!(d.day(), 29);
    /// ```
    #[inline]
    pub const fn constant(year: i16, month: i8, day: i8) -> Date {
        if !Year::contains(year) {
            panic!("invalid year");
        }
        if !Month::contains(month) {
            panic!("invalid month");
        }
        if day > itime::days_in_month(year, month) {
            panic!("invalid day");
        }
        let year = Year::new_unchecked(year);
        let month = Month::new_unchecked(month);
        let day = Day::new_unchecked(day);
        Date { year, month, day }
    }

    /// Construct a Gregorian date from an [ISO 8601 week date].
    ///
    /// The [`ISOWeekDate`] type describes itself in more detail, but in
    /// breif, the ISO week date calendar system eschews months in favor of
    /// weeks.
    ///
    /// The minimum and maximum values of an `ISOWeekDate` correspond
    /// precisely to the minimum and maximum values of a `Date`. Therefore,
    /// converting between them is lossless and infallible.
    ///
    /// This routine is equivalent to [`ISOWeekDate::date`]. It is also
    /// available via a `From<ISOWeekDate>` trait implementation for `Date`.
    ///
    /// [ISO 8601 week date]: https://en.wikipedia.org/wiki/ISO_week_date
    ///
    /// # Example
    ///
    /// This shows a number of examples demonstrating the conversion from an
    /// ISO 8601 week date to a Gregorian date.
    ///
    /// ```
    /// use jiff::civil::{Date, ISOWeekDate, Weekday, date};
    ///
    /// let weekdate = ISOWeekDate::new(1994, 52, Weekday::Sunday).unwrap();
    /// let d = Date::from_iso_week_date(weekdate);
    /// assert_eq!(d, date(1995, 1, 1));
    ///
    /// let weekdate = ISOWeekDate::new(1997, 1, Weekday::Tuesday).unwrap();
    /// let d = Date::from_iso_week_date(weekdate);
    /// assert_eq!(d, date(1996, 12, 31));
    ///
    /// let weekdate = ISOWeekDate::new(2020, 1, Weekday::Monday).unwrap();
    /// let d = Date::from_iso_week_date(weekdate);
    /// assert_eq!(d, date(2019, 12, 30));
    ///
    /// let weekdate = ISOWeekDate::new(2024, 10, Weekday::Saturday).unwrap();
    /// let d = Date::from_iso_week_date(weekdate);
    /// assert_eq!(d, date(2024, 3, 9));
    ///
    /// let weekdate = ISOWeekDate::new(9999, 52, Weekday::Friday).unwrap();
    /// let d = Date::from_iso_week_date(weekdate);
    /// assert_eq!(d, date(9999, 12, 31));
    /// ```
    #[inline]
    pub fn from_iso_week_date(weekdate: ISOWeekDate) -> Date {
        let mut days = iso_week_start_from_year(weekdate.year_ranged());
        let year = t::NoUnits16::rfrom(weekdate.year_ranged());
        let week = t::NoUnits16::rfrom(weekdate.week_ranged());
        let weekday = t::NoUnits16::rfrom(
            weekdate.weekday().to_monday_zero_offset_ranged(),
        );
        let [week, weekday] = t::NoUnits16::vary_many(
            [year, week, weekday],
            |[year, week, weekday]| {
                // This is weird, but because the max ISO week date is actually
                // 9999-W52-4, we need to explicitly cap our maximum computed
                // values here. This is only required because the maximums of
                // each component of an ISO week date combine to represent an
                // out-of-bounds Gregorian date.
                //
                // Note that this is purely done at the service of ranged
                // integers. Otherwise, our ranged integers will compute a
                // max value bigger than what can really occur, and then panic.
                // So we use these caps to say, "no range integer, it truly
                // won't exceed 9999-W52-4."
                if year == C(9999) {
                    if week >= C(52) {
                        [week.min(C(52)), weekday.min(C(4))]
                    } else {
                        [week, weekday]
                    }
                } else {
                    [week, weekday]
                }
            },
        );
        days += (UnixEpochDay::rfrom(week) - C(1)) * C(7);
        days += weekday;
        Date::from_unix_epoch_day(days)
    }

    /// Create a builder for constructing a `Date` from the fields of this
    /// date.
    ///
    /// See the methods on [`DateWith`] for the different ways one can set the
    /// fields of a new `Date`.
    ///
    /// # Example
    ///
    /// The builder ensures one can chain together the individual components
    /// of a date without it failing at an intermediate step. For example,
    /// if you had a date of `2024-10-31` and wanted to change both the day
    /// and the month, and each setting was validated independent of the other,
    /// you would need to be careful to set the day first and then the month.
    /// In some cases, you would need to set the month first and then the day!
    ///
    /// But with the builder, you can set values in any order:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 10, 31);
    /// let d2 = d1.with().month(11).day(30).build()?;
    /// assert_eq!(d2, date(2024, 11, 30));
    ///
    /// let d1 = date(2024, 4, 30);
    /// let d2 = d1.with().day(31).month(7).build()?;
    /// assert_eq!(d2, date(2024, 7, 31));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with(self) -> DateWith {
        DateWith::new(self)
    }

    /// Returns the year for this date.
    ///
    /// The value returned is guaranteed to be in the range `-9999..=9999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 3, 9);
    /// assert_eq!(d1.year(), 2024);
    ///
    /// let d2 = date(-2024, 3, 9);
    /// assert_eq!(d2.year(), -2024);
    ///
    /// let d3 = date(0, 3, 9);
    /// assert_eq!(d3.year(), 0);
    /// ```
    #[inline]
    pub fn year(self) -> i16 {
        self.year_ranged().get()
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
    /// let d = date(2024, 10, 3);
    /// assert_eq!(d.era_year(), (2024, Era::CE));
    ///
    /// let d = date(1, 10, 3);
    /// assert_eq!(d.era_year(), (1, Era::CE));
    ///
    /// let d = date(0, 10, 3);
    /// assert_eq!(d.era_year(), (1, Era::BCE));
    ///
    /// let d = date(-1, 10, 3);
    /// assert_eq!(d.era_year(), (2, Era::BCE));
    ///
    /// let d = date(-10, 10, 3);
    /// assert_eq!(d.era_year(), (11, Era::BCE));
    ///
    /// let d = date(-9_999, 10, 3);
    /// assert_eq!(d.era_year(), (10_000, Era::BCE));
    /// ```
    #[inline]
    pub fn era_year(self) -> (i16, Era) {
        let year = self.year_ranged();
        if year >= C(1) {
            (year.get(), Era::CE)
        } else {
            // We specifically ensure our min/max bounds on `Year` always leave
            // room in its representation to add or subtract 1, so this will
            // never fail.
            let year = -t::YearBCE::rfrom(year.min(C(0)));
            let era_year = year + C(1);
            (era_year.get(), Era::BCE)
        }
    }

    /// Returns the month for this date.
    ///
    /// The value returned is guaranteed to be in the range `1..=12`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 3, 9);
    /// assert_eq!(d1.month(), 3);
    /// ```
    #[inline]
    pub fn month(self) -> i8 {
        self.month_ranged().get()
    }

    /// Returns the day for this date.
    ///
    /// The value returned is guaranteed to be in the range `1..=31`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 2, 29);
    /// assert_eq!(d1.day(), 29);
    /// ```
    #[inline]
    pub fn day(self) -> i8 {
        self.day_ranged().get()
    }

    /// Returns the weekday corresponding to this date.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// // The Unix epoch was on a Thursday.
    /// let d1 = date(1970, 1, 1);
    /// assert_eq!(d1.weekday(), Weekday::Thursday);
    /// // One can also get the weekday as an offset in a variety of schemes.
    /// assert_eq!(d1.weekday().to_monday_zero_offset(), 3);
    /// assert_eq!(d1.weekday().to_monday_one_offset(), 4);
    /// assert_eq!(d1.weekday().to_sunday_zero_offset(), 4);
    /// assert_eq!(d1.weekday().to_sunday_one_offset(), 5);
    /// ```
    #[inline]
    pub fn weekday(self) -> Weekday {
        Weekday::from_iweekday(self.to_idate_const().weekday())
    }

    /// Returns the ordinal day of the year that this date resides in.
    ///
    /// For leap years, this always returns a value in the range `1..=366`.
    /// Otherwise, the value is in the range `1..=365`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2006, 8, 24);
    /// assert_eq!(d.day_of_year(), 236);
    ///
    /// let d = date(2023, 12, 31);
    /// assert_eq!(d.day_of_year(), 365);
    ///
    /// let d = date(2024, 12, 31);
    /// assert_eq!(d.day_of_year(), 366);
    /// ```
    #[inline]
    pub fn day_of_year(self) -> i16 {
        static DAYS_BY_MONTH_NO_LEAP: [i16; 14] =
            [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
        static DAYS_BY_MONTH_LEAP: [i16; 14] =
            [0, 0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335, 366];
        static TABLES: [[i16; 14]; 2] =
            [DAYS_BY_MONTH_NO_LEAP, DAYS_BY_MONTH_LEAP];
        TABLES[self.in_leap_year() as usize][self.month() as usize]
            + i16::from(self.day())
    }

    /// Returns the ordinal day of the year that this date resides in, but
    /// ignores leap years.
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
    /// let d = date(2006, 8, 24);
    /// assert_eq!(d.day_of_year_no_leap(), Some(236));
    ///
    /// let d = date(2023, 12, 31);
    /// assert_eq!(d.day_of_year_no_leap(), Some(365));
    ///
    /// let d = date(2024, 12, 31);
    /// assert_eq!(d.day_of_year_no_leap(), Some(365));
    ///
    /// let d = date(2024, 2, 29);
    /// assert_eq!(d.day_of_year_no_leap(), None);
    /// ```
    #[inline]
    pub fn day_of_year_no_leap(self) -> Option<i16> {
        let mut days = self.day_of_year();
        if self.in_leap_year() {
            // day=60 is Feb 29
            if days == 60 {
                return None;
            } else if days > 60 {
                days -= 1;
            }
        }
        Some(days)
    }

    /// Returns the first date of the month that this date resides in.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 2, 29);
    /// assert_eq!(d.first_of_month(), date(2024, 2, 1));
    /// ```
    #[inline]
    pub fn first_of_month(self) -> Date {
        Date::new_ranged_unchecked(
            self.year_ranged(),
            self.month_ranged(),
            C(1).rinto(),
        )
    }

    /// Returns the last date of the month that this date resides in.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 2, 5);
    /// assert_eq!(d.last_of_month(), date(2024, 2, 29));
    /// ```
    #[inline]
    pub fn last_of_month(self) -> Date {
        let max_day = self.days_in_month_ranged();
        Date::new_ranged_unchecked(
            self.year_ranged(),
            self.month_ranged(),
            max_day,
        )
    }

    /// Returns the total number of days in the the month in which this date
    /// resides.
    ///
    /// This is guaranteed to always return one of the following values,
    /// depending on the year and the month: 28, 29, 30 or 31.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 2, 10);
    /// assert_eq!(d.days_in_month(), 29);
    ///
    /// let d = date(2023, 2, 10);
    /// assert_eq!(d.days_in_month(), 28);
    ///
    /// let d = date(2024, 8, 15);
    /// assert_eq!(d.days_in_month(), 31);
    /// ```
    #[inline]
    pub fn days_in_month(self) -> i8 {
        self.days_in_month_ranged().get()
    }

    /// Returns the first date of the year that this date resides in.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 2, 29);
    /// assert_eq!(d.first_of_year(), date(2024, 1, 1));
    /// ```
    #[inline]
    pub fn first_of_year(self) -> Date {
        Date::new_ranged_unchecked(
            self.year_ranged(),
            C(1).rinto(),
            C(1).rinto(),
        )
    }

    /// Returns the last date of the year that this date resides in.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 2, 5);
    /// assert_eq!(d.last_of_year(), date(2024, 12, 31));
    /// ```
    #[inline]
    pub fn last_of_year(self) -> Date {
        Date::new_ranged_unchecked(
            self.year_ranged(),
            C(12).rinto(),
            C(31).rinto(),
        )
    }

    /// Returns the total number of days in the the year in which this date
    /// resides.
    ///
    /// This is guaranteed to always return either `365` or `366`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 7, 10);
    /// assert_eq!(d.days_in_year(), 366);
    ///
    /// let d = date(2023, 7, 10);
    /// assert_eq!(d.days_in_year(), 365);
    /// ```
    #[inline]
    pub fn days_in_year(self) -> i16 {
        if self.in_leap_year() {
            366
        } else {
            365
        }
    }

    /// Returns true if and only if the year in which this date resides is a
    /// leap year.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// assert!(date(2024, 1, 1).in_leap_year());
    /// assert!(!date(2023, 12, 31).in_leap_year());
    /// ```
    #[inline]
    pub fn in_leap_year(self) -> bool {
        itime::is_leap_year(self.year_ranged().get())
    }

    /// Returns the date immediately following this one.
    ///
    /// # Errors
    ///
    /// This returns an error when this date is the maximum value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{Date, date};
    ///
    /// let d = date(2024, 2, 28);
    /// assert_eq!(d.tomorrow()?, date(2024, 2, 29));
    ///
    /// // The max doesn't have a tomorrow.
    /// assert!(Date::MAX.tomorrow().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn tomorrow(self) -> Result<Date, Error> {
        if self.day() >= 28 && self.day() == self.days_in_month() {
            if self.month() == 12 {
                let year = self.year_ranged().try_checked_add("year", C(1))?;
                let month = Month::new_unchecked(1);
                let day = Day::new_unchecked(1);
                return Ok(Date::new_ranged_unchecked(year, month, day));
            }
            let year = self.year_ranged();
            let month = Month::new_unchecked(self.month() + 1);
            let day = Day::new_unchecked(1);
            return Ok(Date::new_ranged_unchecked(year, month, day));
        }
        let year = self.year_ranged();
        let month = self.month_ranged();
        let day = Day::new_unchecked(self.day() + 1);
        Ok(Date::new_ranged_unchecked(year, month, day))
    }

    /// Returns the date immediately preceding this one.
    ///
    /// # Errors
    ///
    /// This returns an error when this date is the minimum value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{Date, date};
    ///
    /// let d = date(2024, 3, 1);
    /// assert_eq!(d.yesterday()?, date(2024, 2, 29));
    ///
    /// // The min doesn't have a yesterday.
    /// assert!(Date::MIN.yesterday().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn yesterday(self) -> Result<Date, Error> {
        if self.day() == 1 {
            if self.month() == 1 {
                let year = self.year_ranged().try_checked_sub("year", C(1))?;
                let month = Month::new_unchecked(12);
                let day = Day::new_unchecked(31);
                return Ok(Date::new_ranged_unchecked(year, month, day));
            }
            let year = self.year_ranged();
            let month = Month::new_unchecked(self.month() - 1);
            let day = days_in_month(year, month);
            return Ok(Date::new_ranged_unchecked(year, month, day));
        }
        let year = self.year_ranged();
        let month = self.month_ranged();
        let day = Day::new_unchecked(self.day() - 1);
        Ok(Date::new_ranged_unchecked(year, month, day))
    }

    /// Returns the "nth" weekday from the beginning or end of the month in
    /// which this date resides.
    ///
    /// The `nth` parameter can be positive or negative. A positive value
    /// computes the "nth" weekday from the beginning of the month. A negative
    /// value computes the "nth" weekday from the end of the month. So for
    /// example, use `-1` to "find the last weekday" in this date's month.
    ///
    /// # Errors
    ///
    /// This returns an error when `nth` is `0`, or if it is `5` or `-5` and
    /// there is no 5th weekday from the beginning or end of the month.
    ///
    /// # Example
    ///
    /// This shows how to get the nth weekday in a month, starting from the
    /// beginning of the month:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let month = date(2017, 3, 1);
    /// let second_friday = month.nth_weekday_of_month(2, Weekday::Friday)?;
    /// assert_eq!(second_friday, date(2017, 3, 10));
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
    /// let month = date(2024, 3, 1);
    /// let last_thursday = month.nth_weekday_of_month(-1, Weekday::Thursday)?;
    /// assert_eq!(last_thursday, date(2024, 3, 28));
    /// let second_last_thursday = month.nth_weekday_of_month(
    ///     -2,
    ///     Weekday::Thursday,
    /// )?;
    /// assert_eq!(second_last_thursday, date(2024, 3, 21));
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
    /// let month = date(2024, 3, 25);
    /// let fourth_monday = month.nth_weekday_of_month(4, Weekday::Monday)?;
    /// assert_eq!(fourth_monday, date(2024, 3, 25));
    /// // There is no 5th Monday.
    /// assert!(month.nth_weekday_of_month(5, Weekday::Monday).is_err());
    /// // Same goes for counting backwards.
    /// assert!(month.nth_weekday_of_month(-5, Weekday::Monday).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nth_weekday_of_month(
        self,
        nth: i8,
        weekday: Weekday,
    ) -> Result<Date, Error> {
        let weekday = weekday.to_iweekday();
        let idate = self.to_idate_const();
        Ok(Date::from_idate_const(
            idate.nth_weekday_of_month(nth, weekday).map_err(Error::shared)?,
        ))
    }

    /// Returns the "nth" weekday from this date, not including itself.
    ///
    /// The `nth` parameter can be positive or negative. A positive value
    /// computes the "nth" weekday starting at the day after this date and
    /// going forwards in time. A negative value computes the "nth" weekday
    /// starting at the day before this date and going backwards in time.
    ///
    /// For example, if this date's weekday is a Sunday and the first Sunday is
    /// asked for (that is, `date.nth_weekday(1, Weekday::Sunday)`), then the
    /// result is a week from this date corresponding to the following Sunday.
    ///
    /// # Errors
    ///
    /// This returns an error when `nth` is `0`, or if it would otherwise
    /// result in a date that overflows the minimum/maximum values of `Date`.
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
    /// let d = date(2024, 3, 10);
    /// assert_eq!(d.weekday(), Weekday::Sunday);
    ///
    /// // The first next Monday is tomorrow!
    /// let next_monday = d.nth_weekday(1, Weekday::Monday)?;
    /// assert_eq!(next_monday, date(2024, 3, 11));
    ///
    /// // But the next Sunday is a week away, because this doesn't
    /// // include the current weekday.
    /// let next_sunday = d.nth_weekday(1, Weekday::Sunday)?;
    /// assert_eq!(next_sunday, date(2024, 3, 17));
    ///
    /// // "not this Thursday, but next Thursday"
    /// let next_next_thursday = d.nth_weekday(2, Weekday::Thursday)?;
    /// assert_eq!(next_next_thursday, date(2024, 3, 21));
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
    /// let d = date(2024, 3, 10);
    /// assert_eq!(d.weekday(), Weekday::Sunday);
    ///
    /// // "last Saturday" was yesterday!
    /// let last_saturday = d.nth_weekday(-1, Weekday::Saturday)?;
    /// assert_eq!(last_saturday, date(2024, 3, 9));
    ///
    /// // "last Sunday" was a week ago.
    /// let last_sunday = d.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(last_sunday, date(2024, 3, 3));
    ///
    /// // "not last Thursday, but the one before"
    /// let prev_prev_thursday = d.nth_weekday(-2, Weekday::Thursday)?;
    /// assert_eq!(prev_prev_thursday, date(2024, 2, 29));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example shows that overflow results in an error in either
    /// direction:
    ///
    /// ```
    /// use jiff::civil::{Date, Weekday};
    ///
    /// let d = Date::MAX;
    /// assert_eq!(d.weekday(), Weekday::Friday);
    /// assert!(d.nth_weekday(1, Weekday::Saturday).is_err());
    ///
    /// let d = Date::MIN;
    /// assert_eq!(d.weekday(), Weekday::Monday);
    /// assert!(d.nth_weekday(-1, Weekday::Sunday).is_err());
    /// ```
    ///
    /// # Example: the start of Israeli summer time
    ///
    /// Israeli law says (at present, as of 2024-03-11) that DST or "summer
    /// time" starts on the Friday before the last Sunday in March. We can find
    /// that date using both `nth_weekday` and [`Date::nth_weekday_of_month`]:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let march = date(2024, 3, 1);
    /// let last_sunday = march.nth_weekday_of_month(-1, Weekday::Sunday)?;
    /// let dst_starts_on = last_sunday.nth_weekday(-1, Weekday::Friday)?;
    /// assert_eq!(dst_starts_on, date(2024, 3, 29));
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
    /// let d = date(2024, 3, 15);
    /// // For weeks starting with Sunday.
    /// let start_of_week = d.tomorrow()?.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(start_of_week, date(2024, 3, 10));
    /// // For weeks starting with Monday.
    /// let start_of_week = d.tomorrow()?.nth_weekday(-1, Weekday::Monday)?;
    /// assert_eq!(start_of_week, date(2024, 3, 11));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// In the above example, we first get the date after the current one
    /// because `nth_weekday` does not consider itself when counting. This
    /// works as expected even at the boundaries of a week:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// // The start of the week.
    /// let d = date(2024, 3, 10);
    /// let start_of_week = d.tomorrow()?.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(start_of_week, date(2024, 3, 10));
    /// // The end of the week.
    /// let d = date(2024, 3, 16);
    /// let start_of_week = d.tomorrow()?.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(start_of_week, date(2024, 3, 10));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nth_weekday(
        self,
        nth: i32,
        weekday: Weekday,
    ) -> Result<Date, Error> {
        // ref: http://howardhinnant.github.io/date_algorithms.html#next_weekday

        let nth = t::SpanWeeks::try_new("nth weekday", nth)?;
        if nth == C(0) {
            Err(err!("nth weekday cannot be `0`"))
        } else if nth > C(0) {
            let nth = nth.max(C(1));
            let weekday_diff = weekday.since_ranged(self.weekday().next());
            let diff = (nth - C(1)) * C(7) + weekday_diff;
            let start = self.tomorrow()?.to_unix_epoch_day();
            let end = start.try_checked_add("days", diff)?;
            Ok(Date::from_unix_epoch_day(end))
        } else {
            let nth: t::SpanWeeks = nth.min(C(-1)).abs();
            let weekday_diff = self.weekday().previous().since_ranged(weekday);
            let diff = (nth - C(1)) * C(7) + weekday_diff;
            let start = self.yesterday()?.to_unix_epoch_day();
            let end = start.try_checked_sub("days", diff)?;
            Ok(Date::from_unix_epoch_day(end))
        }
    }

    /// Construct an [ISO 8601 week date] from this Gregorian date.
    ///
    /// The [`ISOWeekDate`] type describes itself in more detail, but in
    /// brief, the ISO week date calendar system eschews months in favor of
    /// weeks.
    ///
    /// The minimum and maximum values of an `ISOWeekDate` correspond
    /// precisely to the minimum and maximum values of a `Date`. Therefore,
    /// converting between them is lossless and infallible.
    ///
    /// This routine is equivalent to [`ISOWeekDate::from_date`].
    ///
    /// [ISO 8601 week date]: https://en.wikipedia.org/wiki/ISO_week_date
    ///
    /// # Example
    ///
    /// This shows a number of examples demonstrating the conversion from a
    /// Gregorian date to an ISO 8601 week date:
    ///
    /// ```
    /// use jiff::civil::{Date, Weekday, date};
    ///
    /// let weekdate = date(1995, 1, 1).iso_week_date();
    /// assert_eq!(weekdate.year(), 1994);
    /// assert_eq!(weekdate.week(), 52);
    /// assert_eq!(weekdate.weekday(), Weekday::Sunday);
    ///
    /// let weekdate = date(1996, 12, 31).iso_week_date();
    /// assert_eq!(weekdate.year(), 1997);
    /// assert_eq!(weekdate.week(), 1);
    /// assert_eq!(weekdate.weekday(), Weekday::Tuesday);
    ///
    /// let weekdate = date(2019, 12, 30).iso_week_date();
    /// assert_eq!(weekdate.year(), 2020);
    /// assert_eq!(weekdate.week(), 1);
    /// assert_eq!(weekdate.weekday(), Weekday::Monday);
    ///
    /// let weekdate = date(2024, 3, 9).iso_week_date();
    /// assert_eq!(weekdate.year(), 2024);
    /// assert_eq!(weekdate.week(), 10);
    /// assert_eq!(weekdate.weekday(), Weekday::Saturday);
    ///
    /// let weekdate = Date::MIN.iso_week_date();
    /// assert_eq!(weekdate.year(), -9999);
    /// assert_eq!(weekdate.week(), 1);
    /// assert_eq!(weekdate.weekday(), Weekday::Monday);
    ///
    /// let weekdate = Date::MAX.iso_week_date();
    /// assert_eq!(weekdate.year(), 9999);
    /// assert_eq!(weekdate.week(), 52);
    /// assert_eq!(weekdate.weekday(), Weekday::Friday);
    /// ```
    #[inline]
    pub fn iso_week_date(self) -> ISOWeekDate {
        let days = t::NoUnits32::rfrom(self.to_unix_epoch_day());
        let year = t::NoUnits32::rfrom(self.year_ranged());
        let week_start = t::NoUnits32::vary([days, year], |[days, year]| {
            let mut week_start =
                t::NoUnits32::rfrom(iso_week_start_from_year(year.rinto()));
            if days < week_start {
                week_start = t::NoUnits32::rfrom(iso_week_start_from_year(
                    (year - C(1)).rinto(),
                ));
            } else {
                let next_year_week_start = t::NoUnits32::rfrom(
                    iso_week_start_from_year((year + C(1)).rinto()),
                );
                if days >= next_year_week_start {
                    week_start = next_year_week_start;
                }
            }
            week_start
        });

        let weekday = Weekday::from_iweekday(
            IEpochDay { epoch_day: days.get() }.weekday(),
        );
        let week = ((days - week_start) / C(7)) + C(1);

        let unix_epoch_day = week_start
            + t::NoUnits32::rfrom(
                Weekday::Thursday.since_ranged(Weekday::Monday),
            );
        let year =
            Date::from_unix_epoch_day(unix_epoch_day.rinto()).year_ranged();
        ISOWeekDate::new_ranged(year, week, weekday)
            .expect("all Dates infallibly convert to ISOWeekDates")
    }

    /// Converts a civil date to a [`Zoned`] datetime by adding the given
    /// time zone and setting the clock time to midnight.
    ///
    /// This is a convenience function for
    /// `date.to_datetime(midnight).in_tz(name)`. See [`DateTime::to_zoned`]
    /// for more details. Note that ambiguous datetimes are handled in the
    /// same way as `DateTime::to_zoned`.
    ///
    /// # Errors
    ///
    /// This returns an error when the given time zone name could not be found
    /// in the default time zone database.
    ///
    /// This also returns an error if this date could not be represented as
    /// a timestamp. This can occur in some cases near the minimum and maximum
    /// boundaries of a `Date`.
    ///
    /// # Example
    ///
    /// This is a simple example of converting a civil date (a "wall" or
    /// "local" or "naive" date) to a precise instant in time that is aware of
    /// its time zone:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 6, 20).in_tz("America/New_York")?;
    /// assert_eq!(zdt.to_string(), "2024-06-20T00:00:00-04:00[America/New_York]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: dealing with ambiguity
    ///
    /// Since a [`Zoned`] corresponds to a precise instant in time (to
    /// nanosecond precision) and a `Date` can be many possible such instants,
    /// this routine chooses one for this date: the first one, or midnight.
    ///
    /// Interestingly, some regions implement their daylight saving time
    /// transitions at midnight. This means there are some places in the world
    /// where, once a year, midnight does not exist on their clocks. As a
    /// result, it's possible for the datetime string representing a [`Zoned`]
    /// to be something other than midnight. For example:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 3, 10).in_tz("Cuba")?;
    /// assert_eq!(zdt.to_string(), "2024-03-10T01:00:00-04:00[Cuba]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Since this uses
    /// [`Disambiguation::Compatible`](crate::tz::Disambiguation::Compatible),
    /// and since that also chooses the "later" time in a forward transition,
    /// it follows that the date of the returned `Zoned` will always match
    /// this civil date. (Unless there is a pathological time zone with a 24+
    /// hour transition forward.)
    ///
    /// But if a different disambiguation strategy is used, even when only
    /// dealing with standard one hour transitions, the date returned can be
    /// different:
    ///
    /// ```
    /// use jiff::{civil::date, tz::TimeZone};
    ///
    /// let tz = TimeZone::get("Cuba")?;
    /// let dt = date(2024, 3, 10).at(0, 0, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt).earlier()?;
    /// assert_eq!(zdt.to_string(), "2024-03-09T23:00:00-05:00[Cuba]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn in_tz(self, time_zone_name: &str) -> Result<Zoned, Error> {
        let tz = crate::tz::db().get(time_zone_name)?;
        self.to_zoned(tz)
    }

    /// Converts a civil datetime to a [`Zoned`] datetime by adding the given
    /// [`TimeZone`] and setting the clock time to midnight.
    ///
    /// This is a convenience function for
    /// `date.to_datetime(midnight).to_zoned(tz)`. See [`DateTime::to_zoned`]
    /// for more details. Note that ambiguous datetimes are handled in the same
    /// way as `DateTime::to_zoned`.
    ///
    /// In the common case of a time zone being represented as a name string,
    /// like `Australia/Tasmania`, consider using [`Date::in_tz`]
    /// instead.
    ///
    /// # Errors
    ///
    /// This returns an error if this date could not be represented as a
    /// timestamp. This can occur in some cases near the minimum and maximum
    /// boundaries of a `Date`.
    ///
    /// # Example
    ///
    /// This example shows how to create a zoned value with a fixed time zone
    /// offset:
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::offset(-4).to_time_zone();
    /// let zdt = date(2024, 6, 20).to_zoned(tz)?;
    /// // A time zone annotation is still included in the printable version
    /// // of the Zoned value, but it is fixed to a particular offset.
    /// assert_eq!(zdt.to_string(), "2024-06-20T00:00:00-04:00[-04:00]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_zoned(self, tz: TimeZone) -> Result<Zoned, Error> {
        DateTime::from(self).to_zoned(tz)
    }

    /// Given a [`Time`], this constructs a [`DateTime`] value with its time
    /// component equal to this time.
    ///
    /// This is a convenience function for [`DateTime::from_parts`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{DateTime, date, time};
    ///
    /// let date = date(2010, 3, 14);
    /// let time = time(2, 30, 0, 0);
    /// assert_eq!(DateTime::from_parts(date, time), date.to_datetime(time));
    /// ```
    #[inline]
    pub const fn to_datetime(self, time: Time) -> DateTime {
        DateTime::from_parts(self, time)
    }

    /// A convenience function for constructing a [`DateTime`] from this date
    /// at the time given by its components.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// assert_eq!(
    ///     date(2010, 3, 14).at(2, 30, 0, 0).to_string(),
    ///     "2010-03-14T02:30:00",
    /// );
    /// ```
    ///
    /// One can also flip the order by making use of [`Time::on`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// assert_eq!(
    ///     time(2, 30, 0, 0).on(2010, 3, 14).to_string(),
    ///     "2010-03-14T02:30:00",
    /// );
    /// ```
    #[inline]
    pub const fn at(
        self,
        hour: i8,
        minute: i8,
        second: i8,
        subsec_nanosecond: i32,
    ) -> DateTime {
        DateTime::from_parts(
            self,
            Time::constant(hour, minute, second, subsec_nanosecond),
        )
    }

    /// Add the given span of time to this date. If the sum would overflow the
    /// minimum or maximum date values, then an error is returned.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`DateArithmetic`] type.
    ///
    /// # Properties
    ///
    /// When adding a [`Span`] duration, this routine is _not_ reversible
    /// because some additions may be ambiguous. For example, adding `1 month`
    /// to the date `2024-03-31` will produce `2024-04-30` since April has only
    /// 30 days in a month. Conversely, subtracting `1 month` from `2024-04-30`
    /// will produce `2024-03-30`, which is not the date we started with.
    ///
    /// If spans of time are limited to units of days (or less), then this
    /// routine _is_ reversible. This also implies that all operations with
    /// a [`SignedDuration`] or a [`std::time::Duration`] are reversible.
    ///
    /// # Errors
    ///
    /// If the span added to this date would result in a date that exceeds the
    /// range of a `Date`, then this will return an error.
    ///
    /// # Examples
    ///
    /// This shows a few examples of adding spans of time to various dates.
    /// We make use of the [`ToSpan`](crate::ToSpan) trait for convenient
    /// creation of spans.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert_eq!(d.checked_add(1.months())?, date(2024, 4, 30));
    /// // Adding two months gives us May 31, not May 30.
    /// let d = date(2024, 3, 31);
    /// assert_eq!(d.checked_add(2.months())?, date(2024, 5, 31));
    /// // Any time in the span that does not exceed a day is ignored.
    /// let d = date(2024, 3, 31);
    /// assert_eq!(d.checked_add(23.hours())?, date(2024, 3, 31));
    /// // But if the time exceeds a day, that is accounted for!
    /// let d = date(2024, 3, 31);
    /// assert_eq!(d.checked_add(28.hours())?, date(2024, 4, 1));
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
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert_eq!(d + 1.months(), date(2024, 4, 30));
    /// ```
    ///
    /// # Example: negative spans are supported
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert_eq!(
    ///     d.checked_add(-1.months())?,
    ///     date(2024, 2, 29),
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error on overflow
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert!(d.checked_add(9000.years()).is_err());
    /// assert!(d.checked_add(-19000.years()).is_err());
    /// ```
    ///
    /// # Example: adding absolute durations
    ///
    /// This shows how to add signed and unsigned absolute durations to a
    /// `Date`. Only whole numbers of days are considered. Since this is a
    /// civil date unaware of time zones, days are always 24 hours.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let d = date(2024, 2, 29);
    ///
    /// let dur = SignedDuration::from_hours(24);
    /// assert_eq!(d.checked_add(dur)?, date(2024, 3, 1));
    /// assert_eq!(d.checked_add(-dur)?, date(2024, 2, 28));
    ///
    /// // Any leftover time is truncated. That is, only
    /// // whole days from the duration are considered.
    /// let dur = Duration::from_secs((24 * 60 * 60) + (23 * 60 * 60));
    /// assert_eq!(d.checked_add(dur)?, date(2024, 3, 1));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_add<A: Into<DateArithmetic>>(
        self,
        duration: A,
    ) -> Result<Date, Error> {
        let duration: DateArithmetic = duration.into();
        duration.checked_add(self)
    }

    #[inline]
    fn checked_add_span(self, span: Span) -> Result<Date, Error> {
        if span.is_zero() {
            return Ok(self);
        }
        if span.units().contains_only(Unit::Day) {
            let span_days = span.get_days_ranged();
            return if span_days == C(-1) {
                self.yesterday()
            } else if span_days == C(1) {
                self.tomorrow()
            } else {
                let epoch_days = self.to_unix_epoch_day();
                let days = epoch_days.try_checked_add(
                    "days",
                    UnixEpochDay::rfrom(span.get_days_ranged()),
                )?;
                Ok(Date::from_unix_epoch_day(days))
            };
        }

        let (month, years) =
            month_add_overflowing(self.month, span.get_months_ranged());
        let year = self
            .year
            .try_checked_add("years", years)?
            .try_checked_add("years", span.get_years_ranged())?;
        let date = Date::constrain_ranged(year, month, self.day);
        let epoch_days = date.to_unix_epoch_day();
        let mut days = epoch_days
            .try_checked_add(
                "days",
                C(7) * UnixEpochDay::rfrom(span.get_weeks_ranged()),
            )?
            .try_checked_add(
                "days",
                UnixEpochDay::rfrom(span.get_days_ranged()),
            )?;
        if !span.units().only_time().is_empty() {
            let time_days = span
                .only_lower(Unit::Day)
                .to_invariant_nanoseconds()
                .div_ceil(t::NANOS_PER_CIVIL_DAY);
            days = days.try_checked_add("time", time_days)?;
        }
        Ok(Date::from_unix_epoch_day(days))
    }

    #[inline]
    fn checked_add_duration(
        self,
        duration: SignedDuration,
    ) -> Result<Date, Error> {
        // OK because 24!={-1,0}.
        match duration.as_hours() / 24 {
            0 => Ok(self),
            -1 => self.yesterday(),
            1 => self.tomorrow(),
            days => {
                let days = UnixEpochDay::try_new("days", days).with_context(
                    || {
                        err!(
                            "{days} computed from duration {duration:?} \
                             overflows Jiff's datetime limits",
                        )
                    },
                )?;
                let days =
                    self.to_unix_epoch_day().try_checked_add("days", days)?;
                Ok(Date::from_unix_epoch_day(days))
            }
        }
    }

    /// This routine is identical to [`Date::checked_add`] with the duration
    /// negated.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Date::checked_add`].
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::date, SignedDuration, ToSpan};
    ///
    /// let d = date(2024, 2, 29);
    /// assert_eq!(d.checked_sub(1.year())?, date(2023, 2, 28));
    ///
    /// let dur = SignedDuration::from_hours(24);
    /// assert_eq!(d.checked_sub(dur)?, date(2024, 2, 28));
    ///
    /// let dur = Duration::from_secs(24 * 60 * 60);
    /// assert_eq!(d.checked_sub(dur)?, date(2024, 2, 28));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_sub<A: Into<DateArithmetic>>(
        self,
        duration: A,
    ) -> Result<Date, Error> {
        let duration: DateArithmetic = duration.into();
        duration.checked_neg().and_then(|da| da.checked_add(self))
    }

    /// This routine is identical to [`Date::checked_add`], except the
    /// result saturates on overflow. That is, instead of overflow, either
    /// [`Date::MIN`] or [`Date::MAX`] is returned.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::{Date, date}, SignedDuration, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert_eq!(Date::MAX, d.saturating_add(9000.years()));
    /// assert_eq!(Date::MIN, d.saturating_add(-19000.years()));
    /// assert_eq!(Date::MAX, d.saturating_add(SignedDuration::MAX));
    /// assert_eq!(Date::MIN, d.saturating_add(SignedDuration::MIN));
    /// assert_eq!(Date::MAX, d.saturating_add(std::time::Duration::MAX));
    /// ```
    #[inline]
    pub fn saturating_add<A: Into<DateArithmetic>>(self, duration: A) -> Date {
        let duration: DateArithmetic = duration.into();
        self.checked_add(duration).unwrap_or_else(|_| {
            if duration.is_negative() {
                Date::MIN
            } else {
                Date::MAX
            }
        })
    }

    /// This routine is identical to [`Date::saturating_add`] with the span
    /// parameter negated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::{Date, date}, SignedDuration, ToSpan};
    ///
    /// let d = date(2024, 3, 31);
    /// assert_eq!(Date::MIN, d.saturating_sub(19000.years()));
    /// assert_eq!(Date::MAX, d.saturating_sub(-9000.years()));
    /// assert_eq!(Date::MIN, d.saturating_sub(SignedDuration::MAX));
    /// assert_eq!(Date::MAX, d.saturating_sub(SignedDuration::MIN));
    /// assert_eq!(Date::MIN, d.saturating_sub(std::time::Duration::MAX));
    /// ```
    #[inline]
    pub fn saturating_sub<A: Into<DateArithmetic>>(self, duration: A) -> Date {
        let duration: DateArithmetic = duration.into();
        let Ok(duration) = duration.checked_neg() else { return Date::MIN };
        self.saturating_add(duration)
    }

    /// Returns a span representing the elapsed time from this date until
    /// the given `other` date.
    ///
    /// When `other` occurs before this date, then the span returned will be
    /// negative.
    ///
    /// Depending on the input provided, the span returned is rounded. It may
    /// also be balanced up to bigger units than the default. By default, the
    /// span returned is balanced such that the biggest and smallest possible
    /// unit is days.
    ///
    /// This operation is configured by providing a [`DateDifference`]
    /// value. Since this routine accepts anything that implements
    /// `Into<DateDifference>`, once can pass a `Date` directly. One
    /// can also pass a `(Unit, Date)`, where `Unit` is treated as
    /// [`DateDifference::largest`].
    ///
    /// # Properties
    ///
    /// It is guaranteed that if the returned span is subtracted from `other`,
    /// and if no rounding is requested, and if the largest unit request is at
    /// most `Unit::Day`, then the original date will be returned.
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
    /// An error can occur if `DateDifference` is misconfigured. For example,
    /// if the smallest unit provided is bigger than the largest unit.
    ///
    /// It is guaranteed that if one provides a date with the default
    /// [`DateDifference`] configuration, then this routine will never fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let earlier = date(2006, 8, 24);
    /// let later = date(2019, 1, 31);
    /// assert_eq!(earlier.until(later)?, 4543.days().fieldwise());
    ///
    /// // Flipping the dates is fine, but you'll get a negative span.
    /// let earlier = date(2006, 8, 24);
    /// let later = date(2019, 1, 31);
    /// assert_eq!(later.until(earlier)?, -4543.days().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: using bigger units
    ///
    /// This example shows how to expand the span returned to bigger units.
    /// This makes use of a `From<(Unit, Date)> for DateDifference` trait
    /// implementation.
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ToSpan};
    ///
    /// let d1 = date(1995, 12, 07);
    /// let d2 = date(2019, 01, 31);
    ///
    /// // The default limits durations to using "days" as the biggest unit.
    /// let span = d1.until(d2)?;
    /// assert_eq!(span.to_string(), "P8456D");
    ///
    /// // But we can ask for units all the way up to years.
    /// let span = d1.until((Unit::Year, d2))?;
    /// assert_eq!(span.to_string(), "P23Y1M24D");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding the result
    ///
    /// This shows how one might find the difference between two dates and
    /// have the result rounded to the nearest month.
    ///
    /// In this case, we need to hand-construct a [`DateDifference`]
    /// in order to gain full configurability.
    ///
    /// ```
    /// use jiff::{civil::{date, DateDifference}, Unit, ToSpan};
    ///
    /// let d1 = date(1995, 12, 07);
    /// let d2 = date(2019, 02, 06);
    ///
    /// let span = d1.until(DateDifference::from(d2).smallest(Unit::Month))?;
    /// assert_eq!(span, 277.months().fieldwise());
    ///
    /// // Or even include years to make the span a bit more comprehensible.
    /// let span = d1.until(
    ///     DateDifference::from(d2)
    ///         .smallest(Unit::Month)
    ///         .largest(Unit::Year),
    /// )?;
    /// // Notice that we are one day shy of 23y2m. Rounding spans computed
    /// // between dates uses truncation by default.
    /// assert_eq!(span, 23.years().months(1).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: units biggers than days inhibit reversibility
    ///
    /// If you ask for units bigger than days, then adding the span
    /// returned to the `other` date is not guaranteed to result in the
    /// original date. For example:
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ToSpan};
    ///
    /// let d1 = date(2024, 3, 2);
    /// let d2 = date(2024, 5, 1);
    ///
    /// let span = d1.until((Unit::Month, d2))?;
    /// assert_eq!(span, 1.month().days(29).fieldwise());
    /// let maybe_original = d2.checked_sub(span)?;
    /// // Not the same as the original datetime!
    /// assert_eq!(maybe_original, date(2024, 3, 3));
    ///
    /// // But in the default configuration, days are always the biggest unit
    /// // and reversibility is guaranteed.
    /// let span = d1.until(d2)?;
    /// assert_eq!(span, 60.days().fieldwise());
    /// let is_original = d2.checked_sub(span)?;
    /// assert_eq!(is_original, d1);
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
    pub fn until<A: Into<DateDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: DateDifference = other.into();
        let span = args.since_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round.relative(self))
        } else {
            Ok(span)
        }
    }

    /// This routine is identical to [`Date::until`], but the order of the
    /// parameters is flipped.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Date::until`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Since the default
    /// configuration is used and because a `Span` can represent the difference
    /// between any two possible dates, it will never panic.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let earlier = date(2006, 8, 24);
    /// let later = date(2019, 1, 31);
    /// assert_eq!(later - earlier, 4543.days().fieldwise());
    /// // Equivalent to:
    /// assert_eq!(later.since(earlier).unwrap(), 4543.days().fieldwise());
    /// ```
    #[inline]
    pub fn since<A: Into<DateDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: DateDifference = other.into();
        let span = -args.since_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round.relative(self))
        } else {
            Ok(span)
        }
    }

    /// Returns an absolute duration representing the elapsed time from this
    /// date until the given `other` date.
    ///
    /// When `other` occurs before this date, then the duration returned will
    /// be negative.
    ///
    /// Unlike [`Date::until`], this returns a duration corresponding to a
    /// 96-bit integer of nanoseconds between two dates. In this case of
    /// computing durations between civil dates where all days are assumed to
    /// be 24 hours long, the duration returned will always be divisible by
    /// 24 hours. (That is, `24 * 60 * 60 * 1_000_000_000` nanoseconds.)
    ///
    /// # Fallibility
    ///
    /// This routine never panics or returns an error. Since there are no
    /// configuration options that can be incorrectly provided, no error is
    /// possible when calling this routine. In contrast, [`Date::until`] can
    /// return an error in some cases due to misconfiguration. But like this
    /// routine, [`Date::until`] never panics or returns an error in its
    /// default configuration.
    ///
    /// # When should I use this versus [`Date::until`]?
    ///
    /// See the type documentation for [`SignedDuration`] for the section on
    /// when one should use [`Span`] and when one should use `SignedDuration`.
    /// In short, use `Span` (and therefore `Date::until`) unless you have a
    /// specific reason to do otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let earlier = date(2006, 8, 24);
    /// let later = date(2019, 1, 31);
    /// assert_eq!(
    ///     earlier.duration_until(later),
    ///     SignedDuration::from_hours(4543 * 24),
    /// );
    /// ```
    ///
    /// # Example: difference with [`Date::until`]
    ///
    /// The main difference between this routine and `Date::until` is that the
    /// latter can return units other than a 96-bit integer of nanoseconds.
    /// While a 96-bit integer of nanoseconds can be converted into other
    /// units like hours, this can only be done for uniform units. (Uniform
    /// units are units for which each individual unit always corresponds to
    /// the same elapsed time regardless of the datetime it is relative to.)
    /// This can't be done for units like years, months or days without a
    /// relative date.
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration, Span, SpanRound, ToSpan, Unit};
    ///
    /// let d1 = date(2024, 1, 1);
    /// let d2 = date(2025, 4, 1);
    ///
    /// let span = d1.until((Unit::Year, d2))?;
    /// assert_eq!(span, 1.year().months(3).fieldwise());
    ///
    /// let duration = d1.duration_until(d2);
    /// assert_eq!(duration, SignedDuration::from_hours(456 * 24));
    /// // There's no way to extract years or months from the signed
    /// // duration like one might extract hours (because every hour
    /// // is the same length). Instead, you actually have to convert
    /// // it to a span and then balance it by providing a relative date!
    /// let options = SpanRound::new().largest(Unit::Year).relative(d1);
    /// let span = Span::try_from(duration)?.round(options)?;
    /// assert_eq!(span, 1.year().months(3).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: getting an unsigned duration
    ///
    /// If you're looking to find the duration between two dates as a
    /// [`std::time::Duration`], you'll need to use this method to get a
    /// [`SignedDuration`] and then convert it to a `std::time::Duration`:
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let d1 = date(2024, 7, 1);
    /// let d2 = date(2024, 8, 1);
    /// let duration = Duration::try_from(d1.duration_until(d2))?;
    /// assert_eq!(duration, Duration::from_secs(31 * 24 * 60 * 60));
    ///
    /// // Note that unsigned durations cannot represent all
    /// // possible differences! If the duration would be negative,
    /// // then the conversion fails:
    /// assert!(Duration::try_from(d2.duration_until(d1)).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn duration_until(self, other: Date) -> SignedDuration {
        SignedDuration::date_until(self, other)
    }

    /// This routine is identical to [`Date::duration_until`], but the order of
    /// the parameters is flipped.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let earlier = date(2006, 8, 24);
    /// let later = date(2019, 1, 31);
    /// assert_eq!(
    ///     later.duration_since(earlier),
    ///     SignedDuration::from_hours(4543 * 24),
    /// );
    /// ```
    #[inline]
    pub fn duration_since(self, other: Date) -> SignedDuration {
        SignedDuration::date_until(other, self)
    }

    /// Return an iterator of periodic dates determined by the given span.
    ///
    /// The given span may be negative, in which case, the iterator will move
    /// backwards through time. The iterator won't stop until either the span
    /// itself overflows, or it would otherwise exceed the minimum or maximum
    /// `Date` value.
    ///
    /// # Example: Halloween day of the week
    ///
    /// As a kid, I always hoped for Halloween to fall on a weekend. With this
    /// program, we can print the day of the week for all Halloweens in the
    /// 2020s.
    ///
    /// ```
    /// use jiff::{civil::{Weekday, date}, ToSpan};
    ///
    /// let start = date(2020, 10, 31);
    /// let mut halloween_days_of_week = vec![];
    /// for halloween in start.series(1.years()).take(10) {
    ///     halloween_days_of_week.push(
    ///         (halloween.year(), halloween.weekday()),
    ///     );
    /// }
    /// assert_eq!(halloween_days_of_week, vec![
    ///     (2020, Weekday::Saturday),
    ///     (2021, Weekday::Sunday),
    ///     (2022, Weekday::Monday),
    ///     (2023, Weekday::Tuesday),
    ///     (2024, Weekday::Thursday),
    ///     (2025, Weekday::Friday),
    ///     (2026, Weekday::Saturday),
    ///     (2027, Weekday::Sunday),
    ///     (2028, Weekday::Tuesday),
    ///     (2029, Weekday::Wednesday),
    /// ]);
    /// ```
    ///
    /// # Example: how many times do I mow the lawn in a year?
    ///
    /// I mow the lawn about every week and a half from the beginning of May
    /// to the end of October. About how many times will I mow the lawn in
    /// 2024?
    ///
    /// ```
    /// use jiff::{ToSpan, civil::date};
    ///
    /// let start = date(2024, 5, 1);
    /// let end = date(2024, 10, 31);
    /// let mows = start
    ///     .series(1.weeks().days(3).hours(12))
    ///     .take_while(|&d| d <= end)
    ///     .count();
    /// assert_eq!(mows, 18);
    /// ```
    ///
    /// # Example: a period less than a day
    ///
    /// Using a period less than a day works, but since this type exists at the
    /// granularity of a day, some dates may be repeated.
    ///
    /// ```
    /// use jiff::{civil::{Date, date}, ToSpan};
    ///
    /// let start = date(2024, 3, 11);
    /// let every_five_hours: Vec<Date> =
    ///     start.series(15.hours()).take(7).collect();
    /// assert_eq!(every_five_hours, vec![
    ///     date(2024, 3, 11),
    ///     date(2024, 3, 11),
    ///     date(2024, 3, 12),
    ///     date(2024, 3, 12),
    ///     date(2024, 3, 13),
    ///     date(2024, 3, 14),
    ///     date(2024, 3, 14),
    /// ]);
    /// ```
    ///
    /// # Example: finding the most recent Friday the 13th
    ///
    /// When did the most recent Friday the 13th occur?
    ///
    /// ```
    /// use jiff::{civil::{Weekday, date}, ToSpan};
    ///
    /// let start = date(2024, 3, 13);
    /// let mut found = None;
    /// for date in start.series(-1.months()) {
    ///     if date.weekday() == Weekday::Friday {
    ///         found = Some(date);
    ///         break;
    ///     }
    /// }
    /// assert_eq!(found, Some(date(2023, 10, 13)));
    /// ```
    #[inline]
    pub fn series(self, period: Span) -> DateSeries {
        DateSeries { start: self, period, step: 0 }
    }
}

/// Parsing and formatting using a "printf"-style API.
impl Date {
    /// Parses a civil date in `input` matching the given `format`.
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
    /// construct a civil date. For example, if an offset wasn't parsed.
    ///
    /// # Example
    ///
    /// This example shows how to parse a civil date:
    ///
    /// ```
    /// use jiff::civil::Date;
    ///
    /// // Parse an American date with a two-digit year.
    /// let date = Date::strptime("%m/%d/%y", "7/14/24")?;
    /// assert_eq!(date.to_string(), "2024-07-14");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strptime(
        format: impl AsRef<[u8]>,
        input: impl AsRef<[u8]>,
    ) -> Result<Date, Error> {
        fmt::strtime::parse(format, input).and_then(|tm| tm.to_date())
    }

    /// Formats this civil date according to the given `format`.
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
    /// This example shows how to format a civil date:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let date = date(2024, 7, 15);
    /// let string = date.strftime("%Y-%m-%d is a %A").to_string();
    /// assert_eq!(string, "2024-07-15 is a Monday");
    /// ```
    #[inline]
    pub fn strftime<'f, F: 'f + ?Sized + AsRef<[u8]>>(
        &self,
        format: &'f F,
    ) -> fmt::strtime::Display<'f> {
        fmt::strtime::Display { fmt: format.as_ref(), tm: (*self).into() }
    }
}

/// Internal APIs using ranged integers.
impl Date {
    #[inline]
    pub(crate) fn new_ranged(
        year: Year,
        month: Month,
        day: Day,
    ) -> Result<Date, Error> {
        if day > C(28) {
            let max_day = days_in_month(year, month);
            if day > max_day {
                return Err(day.to_error_with_bounds("day", 1, max_day));
            }
        }
        Ok(Date::new_ranged_unchecked(year, month, day))
    }

    #[inline]
    pub(crate) fn new_ranged_unchecked(
        year: Year,
        month: Month,
        day: Day,
    ) -> Date {
        Date { year, month, day }
    }

    #[inline]
    fn constrain_ranged(year: Year, month: Month, day: Day) -> Date {
        let (year, month, mut day) =
            (year.rinto(), month.rinto(), day.rinto());
        day = saturate_day_in_month(year, month, day);
        Date { year, month, day }
    }

    #[inline]
    pub(crate) fn year_ranged(self) -> Year {
        self.year
    }

    #[inline]
    pub(crate) fn month_ranged(self) -> Month {
        self.month
    }

    #[inline]
    pub(crate) fn day_ranged(self) -> Day {
        self.day
    }

    #[inline]
    pub(crate) fn days_in_month_ranged(self) -> Day {
        days_in_month(self.year_ranged(), self.month_ranged())
    }

    #[inline]
    pub(crate) fn until_days_ranged(self, other: Date) -> t::SpanDays {
        if self == other {
            return C(0).rinto();
        }
        let start = self.to_unix_epoch_day();
        let end = other.to_unix_epoch_day();
        (end - start).rinto()
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn to_unix_epoch_day(self) -> UnixEpochDay {
        self.to_idate().map(|x| x.to_epoch_day().epoch_day).to_rint()
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn from_unix_epoch_day(epoch_day: UnixEpochDay) -> Date {
        let epoch_day = rangeint::composite!((epoch_day) => {
            IEpochDay { epoch_day }
        });
        Date::from_idate(epoch_day.map(|x| x.to_date()))
    }

    #[inline]
    pub(crate) fn to_idate(&self) -> Composite<IDate> {
        rangeint::composite! {
            (year = self.year, month = self.month, day = self.day) => {
                IDate { year, month, day }
            }
        }
    }

    #[inline]
    pub(crate) fn from_idate(idate: Composite<IDate>) -> Date {
        let (year, month, day) =
            rangeint::uncomposite!(idate, c => (c.year, c.month, c.day));
        Date {
            year: year.to_rint(),
            month: month.to_rint(),
            day: day.to_rint(),
        }
    }

    #[inline]
    pub(crate) const fn to_idate_const(self) -> IDate {
        IDate {
            year: self.year.get_unchecked(),
            month: self.month.get_unchecked(),
            day: self.day.get_unchecked(),
        }
    }

    #[inline]
    pub(crate) const fn from_idate_const(idate: IDate) -> Date {
        Date {
            year: Year::new_unchecked(idate.year),
            month: Month::new_unchecked(idate.month),
            day: Day::new_unchecked(idate.day),
        }
    }
}

impl Eq for Date {}

impl PartialEq for Date {
    #[inline]
    fn eq(&self, other: &Date) -> bool {
        // We roll our own PartialEq impl so that we call 'get' on the
        // underlying ranged integer. This forces bugs in boundary conditions
        // to result in panics when 'debug_assertions' is enabled.
        self.day.get() == other.day.get()
            && self.month.get() == other.month.get()
            && self.year.get() == other.year.get()
    }
}

impl Ord for Date {
    #[inline]
    fn cmp(&self, other: &Date) -> core::cmp::Ordering {
        (self.year.get(), self.month.get(), self.day.get()).cmp(&(
            other.year.get(),
            other.month.get(),
            other.day.get(),
        ))
    }
}

impl PartialOrd for Date {
    #[inline]
    fn partial_cmp(&self, other: &Date) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for Date {
    fn default() -> Date {
        Date::ZERO
    }
}

impl core::fmt::Debug for Date {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

impl core::fmt::Display for Date {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        DEFAULT_DATETIME_PRINTER
            .print_date(self, StdFmtWrite(f))
            .map_err(|_| core::fmt::Error)
    }
}

impl core::str::FromStr for Date {
    type Err = Error;

    fn from_str(string: &str) -> Result<Date, Error> {
        DEFAULT_DATETIME_PARSER.parse_date(string)
    }
}

impl From<ISOWeekDate> for Date {
    #[inline]
    fn from(weekdate: ISOWeekDate) -> Date {
        Date::from_iso_week_date(weekdate)
    }
}

impl From<DateTime> for Date {
    #[inline]
    fn from(dt: DateTime) -> Date {
        dt.date()
    }
}

impl From<Zoned> for Date {
    #[inline]
    fn from(zdt: Zoned) -> Date {
        zdt.datetime().date()
    }
}

impl<'a> From<&'a Zoned> for Date {
    #[inline]
    fn from(zdt: &'a Zoned) -> Date {
        zdt.datetime().date()
    }
}

/// Adds a span of time to a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::Add<Span> for Date {
    type Output = Date;

    #[inline]
    fn add(self, rhs: Span) -> Date {
        self.checked_add(rhs).expect("adding span to date overflowed")
    }
}

/// Adds a span of time to a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::AddAssign<Span> for Date {
    #[inline]
    fn add_assign(&mut self, rhs: Span) {
        *self = *self + rhs;
    }
}

/// Subtracts a span of time from a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::Sub<Span> for Date {
    type Output = Date;

    #[inline]
    fn sub(self, rhs: Span) -> Date {
        self.checked_sub(rhs).expect("subing span to date overflowed")
    }
}

/// Subtracts a span of time from a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::SubAssign<Span> for Date {
    #[inline]
    fn sub_assign(&mut self, rhs: Span) {
        *self = *self - rhs;
    }
}

/// Computes the span of time between two dates.
///
/// This will return a negative span when the date being subtracted is greater.
///
/// Since this uses the default configuration for calculating a span between
/// two date (no rounding and largest units is days), this will never panic or
/// fail in any way.
///
/// To configure the largest unit or enable rounding, use [`Date::since`].
impl core::ops::Sub for Date {
    type Output = Span;

    #[inline]
    fn sub(self, rhs: Date) -> Span {
        self.since(rhs).expect("since never fails when given Date")
    }
}

/// Adds a signed duration of time to a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::Add<SignedDuration> for Date {
    type Output = Date;

    #[inline]
    fn add(self, rhs: SignedDuration) -> Date {
        self.checked_add(rhs)
            .expect("adding signed duration to date overflowed")
    }
}

/// Adds a signed duration of time to a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::AddAssign<SignedDuration> for Date {
    #[inline]
    fn add_assign(&mut self, rhs: SignedDuration) {
        *self = *self + rhs;
    }
}

/// Subtracts a signed duration of time from a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::Sub<SignedDuration> for Date {
    type Output = Date;

    #[inline]
    fn sub(self, rhs: SignedDuration) -> Date {
        self.checked_sub(rhs)
            .expect("subing signed duration to date overflowed")
    }
}

/// Subtracts a signed duration of time from a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::SubAssign<SignedDuration> for Date {
    #[inline]
    fn sub_assign(&mut self, rhs: SignedDuration) {
        *self = *self - rhs;
    }
}

/// Adds an unsigned duration of time to a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::Add<UnsignedDuration> for Date {
    type Output = Date;

    #[inline]
    fn add(self, rhs: UnsignedDuration) -> Date {
        self.checked_add(rhs)
            .expect("adding unsigned duration to date overflowed")
    }
}

/// Adds an unsigned duration of time to a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_add`].
impl core::ops::AddAssign<UnsignedDuration> for Date {
    #[inline]
    fn add_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self + rhs;
    }
}

/// Subtracts an unsigned duration of time from a date.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::Sub<UnsignedDuration> for Date {
    type Output = Date;

    #[inline]
    fn sub(self, rhs: UnsignedDuration) -> Date {
        self.checked_sub(rhs)
            .expect("subing unsigned duration to date overflowed")
    }
}

/// Subtracts an unsigned duration of time from a date in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Date::checked_sub`].
impl core::ops::SubAssign<UnsignedDuration> for Date {
    #[inline]
    fn sub_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self - rhs;
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Date {
    #[inline]
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Date {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Date, D::Error> {
        use serde::de;

        struct DateVisitor;

        impl<'de> de::Visitor<'de> for DateVisitor {
            type Value = Date;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str("a date string")
            }

            #[inline]
            fn visit_bytes<E: de::Error>(
                self,
                value: &[u8],
            ) -> Result<Date, E> {
                DEFAULT_DATETIME_PARSER
                    .parse_date(value)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Date, E> {
                self.visit_bytes(value.as_bytes())
            }
        }

        deserializer.deserialize_str(DateVisitor)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Date {
    fn arbitrary(g: &mut quickcheck::Gen) -> Date {
        let year = Year::arbitrary(g);
        let month = Month::arbitrary(g);
        let day = Day::arbitrary(g);
        Date::constrain_ranged(year, month, day)
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Date>> {
        alloc::boxed::Box::new(
            (self.year_ranged(), self.month_ranged(), self.day_ranged())
                .shrink()
                .map(|(year, month, day)| {
                    Date::constrain_ranged(year, month, day)
                }),
        )
    }
}

/// An iterator over periodic dates, created by [`Date::series`].
///
/// It is exhausted when the next value would exceed a [`Span`] or [`Date`]
/// value.
#[derive(Clone, Debug)]
pub struct DateSeries {
    start: Date,
    period: Span,
    step: i64,
}

impl Iterator for DateSeries {
    type Item = Date;

    #[inline]
    fn next(&mut self) -> Option<Date> {
        let span = self.period.checked_mul(self.step).ok()?;
        self.step = self.step.checked_add(1)?;
        let date = self.start.checked_add(span).ok()?;
        Some(date)
    }
}

/// Options for [`Date::checked_add`] and [`Date::checked_sub`].
///
/// This type provides a way to ergonomically add one of a few different
/// duration types to a [`Date`].
///
/// The main way to construct values of this type is with its `From` trait
/// implementations:
///
/// * `From<Span> for DateArithmetic` adds (or subtracts) the given span to the
/// receiver date.
/// * `From<SignedDuration> for DateArithmetic` adds (or subtracts)
/// the given signed duration to the receiver date.
/// * `From<std::time::Duration> for DateArithmetic` adds (or subtracts)
/// the given unsigned duration to the receiver date.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use jiff::{civil::date, SignedDuration, ToSpan};
///
/// let d = date(2024, 2, 29);
/// assert_eq!(d.checked_add(1.year())?, date(2025, 2, 28));
/// assert_eq!(d.checked_add(SignedDuration::from_hours(24))?, date(2024, 3, 1));
/// assert_eq!(d.checked_add(Duration::from_secs(24 * 60 * 60))?, date(2024, 3, 1));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DateArithmetic {
    duration: Duration,
}

impl DateArithmetic {
    #[inline]
    fn checked_add(self, date: Date) -> Result<Date, Error> {
        match self.duration.to_signed()? {
            SDuration::Span(span) => date.checked_add_span(span),
            SDuration::Absolute(sdur) => date.checked_add_duration(sdur),
        }
    }

    #[inline]
    fn checked_neg(self) -> Result<DateArithmetic, Error> {
        let duration = self.duration.checked_neg()?;
        Ok(DateArithmetic { duration })
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.duration.is_negative()
    }
}

impl From<Span> for DateArithmetic {
    fn from(span: Span) -> DateArithmetic {
        let duration = Duration::from(span);
        DateArithmetic { duration }
    }
}

impl From<SignedDuration> for DateArithmetic {
    fn from(sdur: SignedDuration) -> DateArithmetic {
        let duration = Duration::from(sdur);
        DateArithmetic { duration }
    }
}

impl From<UnsignedDuration> for DateArithmetic {
    fn from(udur: UnsignedDuration) -> DateArithmetic {
        let duration = Duration::from(udur);
        DateArithmetic { duration }
    }
}

impl<'a> From<&'a Span> for DateArithmetic {
    fn from(span: &'a Span) -> DateArithmetic {
        DateArithmetic::from(*span)
    }
}

impl<'a> From<&'a SignedDuration> for DateArithmetic {
    fn from(sdur: &'a SignedDuration) -> DateArithmetic {
        DateArithmetic::from(*sdur)
    }
}

impl<'a> From<&'a UnsignedDuration> for DateArithmetic {
    fn from(udur: &'a UnsignedDuration) -> DateArithmetic {
        DateArithmetic::from(*udur)
    }
}

/// Options for [`Date::since`] and [`Date::until`].
///
/// This type provides a way to configure the calculation of spans between two
/// [`Date`] values. In particular, both `Date::since` and `Date::until` accept
/// anything that implements `Into<DateDifference>`. There are a few key trait
/// implementations that make this convenient:
///
/// * `From<Date> for DateDifference` will construct a configuration consisting
/// of just the date. So for example, `date1.until(date2)` will return the span
/// from `date1` to `date2`.
/// * `From<DateTime> for DateDifference` will construct a configuration
/// consisting of just the date from the given datetime. So for example,
/// `date.since(datetime)` returns the span from `datetime.date()` to `date`.
/// * `From<(Unit, Date)>` is a convenient way to specify the largest units
/// that should be present on the span returned. By default, the largest units
/// are days. Using this trait implementation is equivalent to
/// `DateDifference::new(date).largest(unit)`.
/// * `From<(Unit, DateTime)>` is like the one above, but with the date from
/// the given datetime.
///
/// One can also provide a `DateDifference` value directly. Doing so is
/// necessary to use the rounding features of calculating a span. For example,
/// setting the smallest unit (defaults to [`Unit::Day`]), the rounding mode
/// (defaults to [`RoundMode::Trunc`]) and the rounding increment (defaults to
/// `1`). The defaults are selected such that no rounding occurs.
///
/// Rounding a span as part of calculating it is provided as a convenience.
/// Callers may choose to round the span as a distinct step via
/// [`Span::round`], but callers may need to provide a reference date
/// for rounding larger units. By coupling rounding with routines like
/// [`Date::since`], the reference date can be set automatically based on
/// the input to `Date::since`.
///
/// # Example
///
/// This example shows how to round a span between two date to the nearest
/// year, with ties breaking away from zero.
///
/// ```
/// use jiff::{civil::{Date, DateDifference}, RoundMode, ToSpan, Unit};
///
/// let d1 = "2024-03-15".parse::<Date>()?;
/// let d2 = "2030-09-13".parse::<Date>()?;
/// let span = d1.until(
///     DateDifference::new(d2)
///         .smallest(Unit::Year)
///         .mode(RoundMode::HalfExpand),
/// )?;
/// assert_eq!(span, 6.years().fieldwise());
///
/// // If the span were one day longer, it would round up to 7 years.
/// let d2 = "2030-09-14".parse::<Date>()?;
/// let span = d1.until(
///     DateDifference::new(d2)
///         .smallest(Unit::Year)
///         .mode(RoundMode::HalfExpand),
/// )?;
/// assert_eq!(span, 7.years().fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DateDifference {
    date: Date,
    round: SpanRound<'static>,
}

impl DateDifference {
    /// Create a new default configuration for computing the span between
    /// the given date and some other date (specified as the receiver in
    /// [`Date::since`] or [`Date::until`]).
    #[inline]
    pub fn new(date: Date) -> DateDifference {
        // We use truncation rounding by default since it seems that's
        // what is generally expected when computing the difference between
        // datetimes.
        //
        // See: https://github.com/tc39/proposal-temporal/issues/1122
        let round = SpanRound::new().mode(RoundMode::Trunc);
        DateDifference { date, round }
    }

    /// Set the smallest units allowed in the span returned.
    ///
    /// When a largest unit is not specified, then the largest unit is
    /// automatically set to be equal to the smallest unit.
    ///
    /// # Errors
    ///
    /// The smallest units must be no greater than the largest units. If this
    /// is violated, then computing a span with this configuration will result
    /// in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two date to the nearest
    /// number of weeks.
    ///
    /// ```
    /// use jiff::{civil::{Date, DateDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let d1 = "2024-03-15".parse::<Date>()?;
    /// let d2 = "2030-11-22".parse::<Date>()?;
    /// let span = d1.until(
    ///     DateDifference::new(d2)
    ///         .smallest(Unit::Week)
    ///         .largest(Unit::Week)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 349.weeks().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> DateDifference {
        DateDifference { round: self.round.smallest(unit), ..self }
    }

    /// Set the largest units allowed in the span returned.
    ///
    /// When a largest unit is not specified, then the largest unit is
    /// automatically set to be equal to the smallest unit. Otherwise, when the
    /// largest unit is not specified, it is set to days.
    ///
    /// Once a largest unit is set, there is no way to change this rounding
    /// configuration back to using the "automatic" default. Instead, callers
    /// must create a new configuration.
    ///
    /// # Errors
    ///
    /// The largest units, when set, must be at least as big as the smallest
    /// units (which defaults to [`Unit::Day`]). If this is violated, then
    /// computing a span with this configuration will result in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two date to units no
    /// bigger than months.
    ///
    /// ```
    /// use jiff::{civil::{Date, DateDifference}, ToSpan, Unit};
    ///
    /// let d1 = "2024-03-15".parse::<Date>()?;
    /// let d2 = "2030-11-22".parse::<Date>()?;
    /// let span = d1.until(
    ///     DateDifference::new(d2).largest(Unit::Month),
    /// )?;
    /// assert_eq!(span, 80.months().days(7).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn largest(self, unit: Unit) -> DateDifference {
        DateDifference { round: self.round.largest(unit), ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::Trunc`] since it's plausible that
    /// rounding "up" in the context of computing the span between two date
    /// could be surprising in a number of cases. The [`RoundMode::HalfExpand`]
    /// mode corresponds to typical rounding you might have learned about in
    /// school. But a variety of other rounding modes exist.
    ///
    /// # Example
    ///
    /// This shows how to always round "up" towards positive infinity.
    ///
    /// ```
    /// use jiff::{civil::{Date, DateDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let d1 = "2024-01-15".parse::<Date>()?;
    /// let d2 = "2024-08-16".parse::<Date>()?;
    /// let span = d1.until(
    ///     DateDifference::new(d2)
    ///         .smallest(Unit::Month)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// // Only 7 months and 1 day elapsed, but we asked to always round up!
    /// assert_eq!(span, 8.months().fieldwise());
    ///
    /// // Since `Ceil` always rounds toward positive infinity, the behavior
    /// // flips for a negative span.
    /// let span = d1.since(
    ///     DateDifference::new(d2)
    ///         .smallest(Unit::Month)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// assert_eq!(span, -7.months().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> DateDifference {
        DateDifference { round: self.round.mode(mode), ..self }
    }

    /// Set the rounding increment for the smallest unit.
    ///
    /// The default value is `1`. Other values permit rounding the smallest
    /// unit to the nearest integer increment specified. For example, if the
    /// smallest unit is set to [`Unit::Month`], then a rounding increment of
    /// `2` would result in rounding in increments of every other month.
    ///
    /// # Example
    ///
    /// This shows how to round the span between two date to the nearest even
    /// month.
    ///
    /// ```
    /// use jiff::{civil::{Date, DateDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let d1 = "2024-01-15".parse::<Date>()?;
    /// let d2 = "2024-08-15".parse::<Date>()?;
    /// let span = d1.until(
    ///     DateDifference::new(d2)
    ///         .smallest(Unit::Month)
    ///         .increment(2)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 8.months().fieldwise());
    ///
    /// // If our second date was just one day less, rounding would truncate
    /// // down to 6 months!
    /// let d2 = "2024-08-14".parse::<Date>()?;
    /// let span = d1.until(
    ///     DateDifference::new(d2)
    ///         .smallest(Unit::Month)
    ///         .increment(2)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 6.months().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> DateDifference {
        DateDifference { round: self.round.increment(increment), ..self }
    }

    /// Returns true if and only if this configuration could change the span
    /// via rounding.
    #[inline]
    fn rounding_may_change_span(&self) -> bool {
        self.round.rounding_may_change_span_ignore_largest()
    }

    /// Returns the span of time since `d1` to the date in this configuration.
    /// The biggest units allowed are determined by the `smallest` and
    /// `largest` settings, but defaults to `Unit::Day`.
    #[inline]
    fn since_with_largest_unit(&self, d1: Date) -> Result<Span, Error> {
        let d2 = self.date;
        let largest = self
            .round
            .get_largest()
            .unwrap_or_else(|| self.round.get_smallest().max(Unit::Day));
        if largest < Unit::Day {
            // This is the only error case when not rounding! Somewhat
            // unfortunate. I did consider making this a panic instead, because
            // we're so close to it being infallible (I think), but I decided
            // that would be too inconsistent with how we handle invalid units
            // in other places. (It's just that, in other places, invalid units
            // are one of a few different kinds of possible errors.)
            //
            // Another option would be to just assume `largest` is `Unit::Day`
            // when it's a smaller unit.
            //
            // Yet another option is to split `Unit` into `DateUnit` and
            // `TimeUnit`, but I found that to be quite awkward (it was the
            // design I started with).
            //
            // NOTE: I take the above back. It's actually possible for the
            // months component to overflow when largest=month.
            return Err(err!(
                "rounding the span between two dates must use days \
                 or bigger for its units, but found {units}",
                units = largest.plural(),
            ));
        }
        if largest <= Unit::Week {
            let mut weeks = t::SpanWeeks::rfrom(C(0));
            let mut days = d1.until_days_ranged(d2);
            if largest == Unit::Week {
                weeks = days.div_ceil(C(7)).rinto();
                days = days.rem_ceil(C(7));
            }
            return Ok(Span::new().weeks_ranged(weeks).days_ranged(days));
        }

        let year1 = d1.year_ranged();
        let month1 = d1.month_ranged();
        let day1 = d1.day_ranged();
        let mut year2 = d2.year_ranged();
        let mut month2 = d2.month_ranged();
        let day2 = d2.day_ranged();

        let mut years =
            t::SpanYears::rfrom(year2) - t::SpanYears::rfrom(year1);
        let mut months =
            t::SpanMonths::rfrom(month2) - t::SpanMonths::rfrom(month1);
        let mut days = t::SpanDays::rfrom(day2) - t::SpanMonths::rfrom(day1);
        if years != C(0) || months != C(0) {
            let sign = if years != C(0) {
                Sign::rfrom(years.signum())
            } else {
                Sign::rfrom(months.signum())
            };
            let mut days_in_month2 =
                t::SpanDays::rfrom(days_in_month(year2, month2));
            let mut day_correct = t::SpanDays::N::<0>();
            if days.signum() == -sign {
                let original_days_in_month1 = days_in_month2;
                let (y, m) = month_add_one(year2, month2, -sign).unwrap();
                year2 = y;
                month2 = m;

                years =
                    t::SpanYears::rfrom(year2) - t::SpanYears::rfrom(year1);
                months = t::SpanMonths::rfrom(month2)
                    - t::SpanMonths::rfrom(month1);
                days_in_month2 = days_in_month(year2, month2).rinto();
                day_correct = if sign < C(0) {
                    -original_days_in_month1
                } else {
                    days_in_month2
                };
            }

            let day0_trunc = t::SpanDays::rfrom(day1.min(days_in_month2));
            days = t::SpanDays::rfrom(day2) - day0_trunc + day_correct;

            if years != C(0) {
                months = t::SpanMonths::rfrom(month2)
                    - t::SpanMonths::rfrom(month1);
                if months.signum() == -sign {
                    let month_correct = if sign < C(0) {
                        -t::MONTHS_PER_YEAR
                    } else {
                        t::MONTHS_PER_YEAR
                    };
                    year2 -= sign;
                    years = t::SpanYears::rfrom(year2)
                        - t::SpanYears::rfrom(year1);

                    months = t::SpanMonths::rfrom(month2)
                        - t::SpanMonths::rfrom(month1)
                        + month_correct;
                }
            }
        }
        if largest == Unit::Month && years != C(0) {
            months = months.try_checked_add(
                "months",
                t::SpanMonths::rfrom(years) * t::MONTHS_PER_YEAR,
            )?;
            years = C(0).rinto();
        }
        Ok(Span::new()
            .years_ranged(years)
            .months_ranged(months)
            .days_ranged(days))
    }
}

impl From<Date> for DateDifference {
    #[inline]
    fn from(date: Date) -> DateDifference {
        DateDifference::new(date)
    }
}

impl From<DateTime> for DateDifference {
    #[inline]
    fn from(dt: DateTime) -> DateDifference {
        DateDifference::from(Date::from(dt))
    }
}

impl From<Zoned> for DateDifference {
    #[inline]
    fn from(zdt: Zoned) -> DateDifference {
        DateDifference::from(Date::from(zdt))
    }
}

impl<'a> From<&'a Zoned> for DateDifference {
    #[inline]
    fn from(zdt: &'a Zoned) -> DateDifference {
        DateDifference::from(zdt.datetime())
    }
}

impl From<(Unit, Date)> for DateDifference {
    #[inline]
    fn from((largest, date): (Unit, Date)) -> DateDifference {
        DateDifference::from(date).largest(largest)
    }
}

impl From<(Unit, DateTime)> for DateDifference {
    #[inline]
    fn from((largest, dt): (Unit, DateTime)) -> DateDifference {
        DateDifference::from((largest, Date::from(dt)))
    }
}

impl From<(Unit, Zoned)> for DateDifference {
    #[inline]
    fn from((largest, zdt): (Unit, Zoned)) -> DateDifference {
        DateDifference::from((largest, Date::from(zdt)))
    }
}

impl<'a> From<(Unit, &'a Zoned)> for DateDifference {
    #[inline]
    fn from((largest, zdt): (Unit, &'a Zoned)) -> DateDifference {
        DateDifference::from((largest, zdt.datetime()))
    }
}

/// A builder for setting the fields on a [`Date`].
///
/// This builder is constructed via [`Date::with`].
///
/// # Example
///
/// The builder ensures one can chain together the individual components
/// of a date without it failing at an intermediate step. For example,
/// if you had a date of `2024-10-31` and wanted to change both the day
/// and the month, and each setting was validated independent of the other,
/// you would need to be careful to set the day first and then the month.
/// In some cases, you would need to set the month first and then the day!
///
/// But with the builder, you can set values in any order:
///
/// ```
/// use jiff::civil::date;
///
/// let d1 = date(2024, 10, 31);
/// let d2 = d1.with().month(11).day(30).build()?;
/// assert_eq!(d2, date(2024, 11, 30));
///
/// let d1 = date(2024, 4, 30);
/// let d2 = d1.with().day(31).month(7).build()?;
/// assert_eq!(d2, date(2024, 7, 31));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DateWith {
    original: Date,
    year: Option<DateWithYear>,
    month: Option<i8>,
    day: Option<DateWithDay>,
}

impl DateWith {
    #[inline]
    fn new(original: Date) -> DateWith {
        DateWith { original, year: None, month: None, day: None }
    }

    /// Create a new `Date` from the fields set on this configuration.
    ///
    /// An error occurs when the fields combine to an invalid date.
    ///
    /// For any fields not set on this configuration, the values are taken from
    /// the [`Date`] that originally created this configuration. When no values
    /// are set, this routine is guaranteed to succeed and will always return
    /// the original date without modification.
    ///
    /// # Example
    ///
    /// This creates a date corresponding to the last day in the year:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// assert_eq!(
    ///     date(2023, 1, 1).with().day_of_year_no_leap(365).build()?,
    ///     date(2023, 12, 31),
    /// );
    /// // It also works with leap years for the same input:
    /// assert_eq!(
    ///     date(2024, 1, 1).with().day_of_year_no_leap(365).build()?,
    ///     date(2024, 12, 31),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error for invalid date
    ///
    /// If the fields combine to form an invalid date, then an error is
    /// returned:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 11, 30);
    /// assert!(d.with().day(31).build().is_err());
    ///
    /// let d = date(2024, 2, 29);
    /// assert!(d.with().year(2023).build().is_err());
    /// ```
    #[inline]
    pub fn build(self) -> Result<Date, Error> {
        let year = match self.year {
            None => self.original.year_ranged(),
            Some(DateWithYear::Jiff(year)) => Year::try_new("year", year)?,
            Some(DateWithYear::EraYear(year, Era::CE)) => {
                let year_ce = t::YearCE::try_new("CE year", year)?;
                t::Year::try_rfrom("CE year", year_ce)?
            }
            Some(DateWithYear::EraYear(year, Era::BCE)) => {
                let year_bce = t::YearBCE::try_new("BCE year", year)?;
                t::Year::try_rfrom("BCE year", -year_bce + C(1))?
            }
        };
        let month = match self.month {
            None => self.original.month_ranged(),
            Some(month) => Month::try_new("month", month)?,
        };
        let day = match self.day {
            None => self.original.day_ranged(),
            Some(DateWithDay::OfMonth(day)) => Day::try_new("day", day)?,
            Some(DateWithDay::OfYear(day)) => {
                let year = year.get_unchecked();
                let idate = IDate::from_day_of_year(year, day)
                    .map_err(Error::shared)?;
                return Ok(Date::from_idate_const(idate));
            }
            Some(DateWithDay::OfYearNoLeap(day)) => {
                let year = year.get_unchecked();
                let idate = IDate::from_day_of_year_no_leap(year, day)
                    .map_err(Error::shared)?;
                return Ok(Date::from_idate_const(idate));
            }
        };
        Date::new_ranged(year, month, day)
    }

    /// Set the year field on a [`Date`].
    ///
    /// One can access this value via [`Date::year`].
    ///
    /// This overrides any previous year settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`DateWith::build`] is called if the given
    /// year is outside the range `-9999..=9999`. This can also return an error
    /// if the resulting date is otherwise invalid.
    ///
    /// # Example
    ///
    /// This shows how to create a new date with a different year:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2005, 11, 5);
    /// assert_eq!(d1.year(), 2005);
    /// let d2 = d1.with().year(2007).build()?;
    /// assert_eq!(d2.year(), 2007);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: only changing the year can fail
    ///
    /// For example, while `2024-02-29` is valid, `2023-02-29` is not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 2, 29);
    /// assert!(d1.with().year(2023).build().is_err());
    /// ```
    #[inline]
    pub fn year(self, year: i16) -> DateWith {
        DateWith { year: Some(DateWithYear::Jiff(year)), ..self }
    }

    /// Set year of a date via its era and its non-negative numeric component.
    ///
    /// One can access this value via [`Date::era_year`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`DateWith::build`] is called if the year is
    /// outside the range for the era specified. For [`Era::BCE`], the range is
    /// `1..=10000`. For [`Era::CE`], the range is `1..=9999`.
    ///
    /// # Example
    ///
    /// This shows that `CE` years are equivalent to the years used by this
    /// crate:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let d1 = date(2005, 11, 5);
    /// assert_eq!(d1.year(), 2005);
    /// let d2 = d1.with().era_year(2007, Era::CE).build()?;
    /// assert_eq!(d2.year(), 2007);
    ///
    /// // CE years are always positive and can be at most 9999:
    /// assert!(d1.with().era_year(-5, Era::CE).build().is_err());
    /// assert!(d1.with().era_year(10_000, Era::CE).build().is_err());
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
    /// let d1 = date(-27, 7, 1);
    /// assert_eq!(d1.year(), -27);
    /// assert_eq!(d1.era_year(), (28, Era::BCE));
    ///
    /// let d2 = d1.with().era_year(509, Era::BCE).build()?;
    /// assert_eq!(d2.year(), -508);
    /// assert_eq!(d2.era_year(), (509, Era::BCE));
    ///
    /// let d2 = d1.with().era_year(10_000, Era::BCE).build()?;
    /// assert_eq!(d2.year(), -9_999);
    /// assert_eq!(d2.era_year(), (10_000, Era::BCE));
    ///
    /// // BCE years are always positive and can be at most 10000:
    /// assert!(d1.with().era_year(-5, Era::BCE).build().is_err());
    /// assert!(d1.with().era_year(10_001, Era::BCE).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: overrides `DateWith::year`
    ///
    /// Setting this option will override any previous `DateWith::year`
    /// option:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let d1 = date(2024, 7, 2);
    /// let d2 = d1.with().year(2000).era_year(1900, Era::CE).build()?;
    /// assert_eq!(d2, date(1900, 7, 2));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Similarly, `DateWith::year` will override any previous call to
    /// `DateWith::era_year`:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let d1 = date(2024, 7, 2);
    /// let d2 = d1.with().era_year(1900, Era::CE).year(2000).build()?;
    /// assert_eq!(d2, date(2000, 7, 2));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn era_year(self, year: i16, era: Era) -> DateWith {
        DateWith { year: Some(DateWithYear::EraYear(year, era)), ..self }
    }

    /// Set the month field on a [`Date`].
    ///
    /// One can access this value via [`Date::month`].
    ///
    /// This overrides any previous month settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`DateWith::build`] is called if the given
    /// month is outside the range `1..=12`. This can also return an error if
    /// the resulting date is otherwise invalid.
    ///
    /// # Example
    ///
    /// This shows how to create a new date with a different month:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2005, 11, 5);
    /// assert_eq!(d1.month(), 11);
    /// let d2 = d1.with().month(6).build()?;
    /// assert_eq!(d2.month(), 6);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: only changing the month can fail
    ///
    /// For example, while `2024-10-31` is valid, `2024-11-31` is not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 10, 31);
    /// assert!(d.with().month(11).build().is_err());
    /// ```
    #[inline]
    pub fn month(self, month: i8) -> DateWith {
        DateWith { month: Some(month), ..self }
    }

    /// Set the day field on a [`Date`].
    ///
    /// One can access this value via [`Date::day`].
    ///
    /// This overrides any previous day settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`DateWith::build`] is called if the given
    /// given day is outside of allowable days for the corresponding year and
    /// month fields.
    ///
    /// # Example
    ///
    /// This shows some examples of setting the day, including a leap day:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d1 = date(2024, 2, 5);
    /// assert_eq!(d1.day(), 5);
    /// let d2 = d1.with().day(10).build()?;
    /// assert_eq!(d2.day(), 10);
    /// let d3 = d1.with().day(29).build()?;
    /// assert_eq!(d3.day(), 29);
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
    /// let d1 = date(2023, 2, 5);
    /// // 2023 is not a leap year
    /// assert!(d1.with().day(29).build().is_err());
    ///
    /// // September has 30 days, not 31.
    /// let d1 = date(2023, 9, 5);
    /// assert!(d1.with().day(31).build().is_err());
    /// ```
    #[inline]
    pub fn day(self, day: i8) -> DateWith {
        DateWith { day: Some(DateWithDay::OfMonth(day)), ..self }
    }

    /// Set the day field on a [`Date`] via the ordinal number of a day within
    /// a year.
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
    /// This returns an error when [`DateWith::build`] is called if the given
    /// day is outside the allowed range of `1..=366`, or when a value of `366`
    /// is given for a non-leap year.
    ///
    /// # Example
    ///
    /// This demonstrates that if a year is a leap year, then `60` corresponds
    /// to February 29:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2024, 1, 1);
    /// assert_eq!(d.with().day_of_year(60).build()?, date(2024, 2, 29));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// But for non-leap years, day 60 is March 1:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let d = date(2023, 1, 1);
    /// assert_eq!(d.with().day_of_year(60).build()?, date(2023, 3, 1));
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
    /// let d = date(2023, 1, 1);
    /// assert!(d.with().day_of_year(366).build().is_err());
    /// // The maximal year is not a leap year, so it returns an error too.
    /// let d = date(9999, 1, 1);
    /// assert!(d.with().day_of_year(366).build().is_err());
    /// ```
    #[inline]
    pub fn day_of_year(self, day: i16) -> DateWith {
        DateWith { day: Some(DateWithDay::OfYear(day)), ..self }
    }

    /// Set the day field on a [`Date`] via the ordinal number of a day within
    /// a year, but ignoring leap years.
    ///
    /// When used, any settings for month are ignored since the month is
    /// determined by the day of the year.
    ///
    /// The valid values for `day` are `1..=365`. The value `365` always
    /// corresponds to the last day of the year, even for leap years. It is
    /// impossible for this routine to return a date corresponding to February
    /// 29.
    ///
    /// This overrides any previous day settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`DateWith::build`] is called if the given
    /// day is outside the allowed range of `1..=365`.
    ///
    /// # Example
    ///
    /// This demonstrates that `60` corresponds to March 1, regardless of
    /// whether the year is a leap year or not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// assert_eq!(
    ///     date(2023, 1, 1).with().day_of_year_no_leap(60).build()?,
    ///     date(2023, 3, 1),
    /// );
    ///
    /// assert_eq!(
    ///     date(2024, 1, 1).with().day_of_year_no_leap(60).build()?,
    ///     date(2024, 3, 1),
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
    /// let d = date(2023, 1, 1);
    /// assert_eq!(
    ///     d.with().day_of_year_no_leap(365).build()?,
    ///     d.last_of_year(),
    /// );
    ///
    /// let d = date(2024, 1, 1);
    /// assert_eq!(
    ///     d.with().day_of_year_no_leap(365).build()?,
    ///     d.last_of_year(),
    /// );
    ///
    /// let d = date(9999, 1, 1);
    /// assert_eq!(
    ///     d.with().day_of_year_no_leap(365).build()?,
    ///     d.last_of_year(),
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
    /// let d = date(2024, 1, 1);
    /// assert!(d.with().day_of_year_no_leap(366).build().is_err());
    /// ```
    #[inline]
    pub fn day_of_year_no_leap(self, day: i16) -> DateWith {
        DateWith { day: Some(DateWithDay::OfYearNoLeap(day)), ..self }
    }
}

/// Encodes the "with year" option of [`DateWith`].
///
/// This encodes the invariant that `DateWith::year` and `DateWith::era_year`
/// are mutually exclusive and override each other.
#[derive(Clone, Copy, Debug)]
enum DateWithYear {
    Jiff(i16),
    EraYear(i16, Era),
}

/// Encodes the "with day" option of [`DateWith`].
///
/// This encodes the invariant that `DateWith::day`, `DateWith::day_of_year`
/// and `DateWith::day_of_year_no_leap` are all mutually exclusive and override
/// each other.
///
/// Note that when "day of year" or "day of year no leap" are used, then if a
/// day of month is set, it is ignored.
#[derive(Clone, Copy, Debug)]
enum DateWithDay {
    OfMonth(i8),
    OfYear(i16),
    OfYearNoLeap(i16),
}

/// Returns the Unix epoch day corresponding to the first day in the ISO 8601
/// week year given.
///
/// Ref: http://howardhinnant.github.io/date_algorithms.html
fn iso_week_start_from_year(year: t::ISOYear) -> UnixEpochDay {
    // A week's year always corresponds to the Gregorian year in which the
    // Thursday of that week falls. Therefore, Jan 4 is *always* in the first
    // week of any ISO week year.
    let date_in_first_week =
        Date::new_ranged(year.rinto(), C(1).rinto(), C(4).rinto())
            .expect("Jan 4 is valid for all valid years");
    // The start of the first week is a Monday, so find the number of days
    // since Monday from a date that we know is in the first ISO week of
    // `year`.
    let diff_from_monday =
        date_in_first_week.weekday().since_ranged(Weekday::Monday);
    date_in_first_week.to_unix_epoch_day() - diff_from_monday
}

/// Adds or subtracts `sign` from the given `year`/`month`.
///
/// If month overflows in either direction, then the `year` returned is
/// adjusted as appropriate.
fn month_add_one(
    mut year: Year,
    mut month: Month,
    delta: Sign,
) -> Result<(Year, Month), Error> {
    month += delta;
    if month < C(1) {
        year -= C(1);
        month += t::MONTHS_PER_YEAR;
    } else if month > t::MONTHS_PER_YEAR {
        year += C(1);
        month -= t::MONTHS_PER_YEAR;
    }
    let year = Year::try_rfrom("year", year)?;
    let month = Month::try_rfrom("year", month)?;
    Ok((year, month))
}

/// Adds the given span of months to the `month` given.
///
/// If adding (or subtracting) would result in overflowing the `month` value,
/// then the amount by which it overflowed, in units of years, is returned. For
/// example, adding 14 months to the month `3` (March) will result in returning
/// the month `5` (May) with `1` year of overflow.
fn month_add_overflowing(
    month: t::Month,
    span: t::SpanMonths,
) -> (t::Month, t::SpanYears) {
    let month = t::SpanMonths::rfrom(month);
    let total = month - C(1) + span;
    let years = total / C(12);
    let month = (total % C(12)) + C(1);
    (month.rinto(), years.rinto())
}

/// Saturates the given day in the month.
///
/// That is, if the day exceeds the maximum number of days in the given year
/// and month, then this returns the maximum. Otherwise, it returns the day
/// given.
#[inline]
fn saturate_day_in_month(year: Year, month: Month, day: Day) -> Day {
    day.min(days_in_month(year, month))
}

/// Returns the number of days in the given year and month.
///
/// This correctly returns `29` when the year is a leap year and the month is
/// February.
#[inline]
fn days_in_month(year: Year, month: Month) -> Day {
    let c = rangeint::composite!((year, month) => {
        itime::days_in_month(year, month)
    });
    c.to_rint()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{civil::date, span::span_eq, tz::TimeZone, Timestamp, ToSpan};

    use super::*;

    #[test]
    fn t_from_unix() {
        fn date_from_timestamp(timestamp: Timestamp) -> Date {
            timestamp.to_zoned(TimeZone::UTC).datetime().date()
        }

        assert_eq!(
            date(1970, 1, 1),
            date_from_timestamp(Timestamp::new(0, 0).unwrap()),
        );
        assert_eq!(
            date(1969, 12, 31),
            date_from_timestamp(Timestamp::new(-1, 0).unwrap()),
        );
        assert_eq!(
            date(1969, 12, 31),
            date_from_timestamp(Timestamp::new(-86_400, 0).unwrap()),
        );
        assert_eq!(
            date(1969, 12, 30),
            date_from_timestamp(Timestamp::new(-86_401, 0).unwrap()),
        );
        assert_eq!(
            date(-9999, 1, 2),
            date_from_timestamp(
                Timestamp::new(t::UnixSeconds::MIN_REPR, 0).unwrap()
            ),
        );
        assert_eq!(
            date(9999, 12, 30),
            date_from_timestamp(
                Timestamp::new(t::UnixSeconds::MAX_REPR, 0).unwrap()
            ),
        );
    }

    #[test]
    #[cfg(not(miri))]
    fn all_days_to_date_roundtrip() {
        for rd in -100_000..=100_000 {
            let rd = UnixEpochDay::new(rd).unwrap();
            let date = Date::from_unix_epoch_day(rd);
            let got = date.to_unix_epoch_day();
            assert_eq!(rd, got, "for date {date:?}");
        }
    }

    #[test]
    #[cfg(not(miri))]
    fn all_date_to_days_roundtrip() {
        let year_range = 2000..=2500;
        // let year_range = -9999..=9999;
        for year in year_range {
            let year = Year::new(year).unwrap();
            for month in Month::MIN_REPR..=Month::MAX_REPR {
                let month = Month::new(month).unwrap();
                for day in 1..=days_in_month(year, month).get() {
                    let date = date(year.get(), month.get(), day);
                    let rd = date.to_unix_epoch_day();
                    let got = Date::from_unix_epoch_day(rd);
                    assert_eq!(date, got, "for date {date:?}");
                }
            }
        }
    }

    #[test]
    #[cfg(not(miri))]
    fn all_date_to_iso_week_date_roundtrip() {
        let year_range = 2000..=2500;
        for year in year_range {
            let year = Year::new(year).unwrap();
            for month in [1, 2, 4] {
                let month = Month::new(month).unwrap();
                for day in 20..=days_in_month(year, month).get() {
                    let date = date(year.get(), month.get(), day);
                    let wd = date.iso_week_date();
                    let got = wd.date();
                    assert_eq!(
                        date, got,
                        "for date {date:?}, and ISO week date {wd:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn add_constrained() {
        use crate::ToSpan;

        let d1 = date(2023, 3, 31);
        let d2 = d1.checked_add(1.months().days(1)).unwrap();
        assert_eq!(d2, date(2023, 5, 1));
    }

    #[test]
    fn since_years() {
        let d1 = date(2023, 4, 15);
        let d2 = date(2019, 2, 22);
        let span = d1.since((Unit::Year, d2)).unwrap();
        span_eq!(span, 4.years().months(1).days(21));
        let span = d2.since((Unit::Year, d1)).unwrap();
        span_eq!(span, -4.years().months(1).days(24));

        let d1 = date(2023, 2, 22);
        let d2 = date(2019, 4, 15);
        let span = d1.since((Unit::Year, d2)).unwrap();
        span_eq!(span, 3.years().months(10).days(7));
        let span = d2.since((Unit::Year, d1)).unwrap();
        span_eq!(span, -3.years().months(10).days(7));

        let d1 = date(9999, 12, 31);
        let d2 = date(-9999, 1, 1);
        let span = d1.since((Unit::Year, d2)).unwrap();
        span_eq!(span, 19998.years().months(11).days(30));
        let span = d2.since((Unit::Year, d1)).unwrap();
        span_eq!(span, -19998.years().months(11).days(30));
    }

    #[test]
    fn since_months() {
        let d1 = date(2024, 7, 24);
        let d2 = date(2024, 2, 22);
        let span = d1.since((Unit::Month, d2)).unwrap();
        span_eq!(span, 5.months().days(2));
        let span = d2.since((Unit::Month, d1)).unwrap();
        span_eq!(span, -5.months().days(2));
        assert_eq!(d2, d1.checked_sub(5.months().days(2)).unwrap());
        assert_eq!(d1, d2.checked_sub(-5.months().days(2)).unwrap());

        let d1 = date(2024, 7, 15);
        let d2 = date(2024, 2, 22);
        let span = d1.since((Unit::Month, d2)).unwrap();
        span_eq!(span, 4.months().days(22));
        let span = d2.since((Unit::Month, d1)).unwrap();
        span_eq!(span, -4.months().days(23));
        assert_eq!(d2, d1.checked_sub(4.months().days(22)).unwrap());
        assert_eq!(d1, d2.checked_sub(-4.months().days(23)).unwrap());

        let d1 = date(2023, 4, 15);
        let d2 = date(2023, 2, 22);
        let span = d1.since((Unit::Month, d2)).unwrap();
        span_eq!(span, 1.month().days(21));
        let span = d2.since((Unit::Month, d1)).unwrap();
        span_eq!(span, -1.month().days(24));
        assert_eq!(d2, d1.checked_sub(1.month().days(21)).unwrap());
        assert_eq!(d1, d2.checked_sub(-1.month().days(24)).unwrap());

        let d1 = date(2023, 4, 15);
        let d2 = date(2019, 2, 22);
        let span = d1.since((Unit::Month, d2)).unwrap();
        span_eq!(span, 49.months().days(21));
        let span = d2.since((Unit::Month, d1)).unwrap();
        span_eq!(span, -49.months().days(24));
    }

    #[test]
    fn since_weeks() {
        let d1 = date(2024, 7, 15);
        let d2 = date(2024, 6, 22);
        let span = d1.since((Unit::Week, d2)).unwrap();
        span_eq!(span, 3.weeks().days(2));
        let span = d2.since((Unit::Week, d1)).unwrap();
        span_eq!(span, -3.weeks().days(2));
    }

    #[test]
    fn since_days() {
        let d1 = date(2024, 7, 15);
        let d2 = date(2024, 2, 22);
        let span = d1.since((Unit::Day, d2)).unwrap();
        span_eq!(span, 144.days());
        let span = d2.since((Unit::Day, d1)).unwrap();
        span_eq!(span, -144.days());
    }

    #[test]
    fn until_month_lengths() {
        let jan1 = date(2020, 1, 1);
        let feb1 = date(2020, 2, 1);
        let mar1 = date(2020, 3, 1);

        span_eq!(jan1.until(feb1).unwrap(), 31.days());
        span_eq!(jan1.until((Unit::Month, feb1)).unwrap(), 1.month());
        span_eq!(feb1.until(mar1).unwrap(), 29.days());
        span_eq!(feb1.until((Unit::Month, mar1)).unwrap(), 1.month());
        span_eq!(jan1.until(mar1).unwrap(), 60.days());
        span_eq!(jan1.until((Unit::Month, mar1)).unwrap(), 2.months());
    }

    // Ref: https://github.com/tc39/proposal-temporal/issues/2845#issuecomment-2121057896
    #[test]
    fn since_until_not_commutative() {
        // Temporal.PlainDate.from("2020-04-30").since("2020-02-29", {largestUnit: "months"})
        // // => P2M
        // Temporal.PlainDate.from("2020-02-29").until("2020-04-30", {largestUnit: "months"})
        // // => P2M1D
        let d1 = date(2020, 4, 30);
        let d2 = date(2020, 2, 29);

        let since = d1.since((Unit::Month, d2)).unwrap();
        span_eq!(since, 2.months());

        let until = d2.until((Unit::Month, d1)).unwrap();
        span_eq!(until, 2.months().days(1));
    }

    // Ref: https://github.com/tc39/proposal-temporal/issues/2893
    #[test]
    fn until_weeks_round() {
        use crate::{RoundMode, SpanRound};

        let earlier = date(2019, 1, 8);
        let later = date(2021, 9, 7);
        let span = earlier.until((Unit::Week, later)).unwrap();
        span_eq!(span, 139.weeks());

        let options = SpanRound::new()
            .smallest(Unit::Week)
            .mode(RoundMode::HalfExpand)
            .relative(earlier.to_datetime(Time::midnight()));
        let rounded = span.round(options).unwrap();
        span_eq!(rounded, 139.weeks());
    }

    // This test checks current behavior, but I think it's wrong. I think the
    // results below should be 11 months and 1 month.
    //
    // Ref: https://github.com/tc39/proposal-temporal/issues/2919
    #[test]
    fn until_months_no_balance() {
        let sp =
            date(2023, 5, 31).until((Unit::Month, date(2024, 4, 30))).unwrap();
        span_eq!(sp, 10.months().days(30));

        let sp =
            date(2023, 5, 31).until((Unit::Month, date(2023, 6, 30))).unwrap();
        span_eq!(sp, 30.days());
    }

    #[test]
    fn test_month_add() {
        let add =
            |year: i16, month: i8, delta: i8| -> Result<(i16, i8), Error> {
                let year = Year::new(year).unwrap();
                let month = Month::new(month).unwrap();
                let delta = Sign::new(delta).unwrap();
                let (year, month) = month_add_one(year, month, delta)?;
                Ok((year.get(), month.get()))
            };

        assert_eq!(add(2024, 1, 1).unwrap(), (2024, 2));
        assert_eq!(add(2024, 1, -1).unwrap(), (2023, 12));
        assert_eq!(add(2024, 12, 1).unwrap(), (2025, 1));
        assert_eq!(add(9999, 12, -1).unwrap(), (9999, 11));
        assert_eq!(add(-9999, 1, 1).unwrap(), (-9999, 2));

        assert!(add(9999, 12, 1).is_err());
        assert!(add(-9999, 1, -1).is_err());
    }

    #[test]
    fn test_month_add_overflowing() {
        let month_add = |month, span| {
            let month = t::Month::new(month).unwrap();
            let span = t::SpanMonths::new(span).unwrap();
            let (month, years) = month_add_overflowing(month, span);
            (month.get(), years.get())
        };

        assert_eq!((1, 0), month_add(1, 0));
        assert_eq!((12, 0), month_add(1, 11));
        assert_eq!((1, 1), month_add(1, 12));
        assert_eq!((2, 1), month_add(1, 13));
        assert_eq!((9, 1), month_add(1, 20));
        assert_eq!((12, 19998), month_add(12, t::SpanMonths::MAX_REPR));

        assert_eq!((12, -1), month_add(1, -1));
        assert_eq!((11, -1), month_add(1, -2));
        assert_eq!((1, -1), month_add(1, -12));
        assert_eq!((12, -2), month_add(1, -13));
    }

    #[test]
    fn date_size() {
        #[cfg(debug_assertions)]
        {
            assert_eq!(12, core::mem::size_of::<Date>());
        }
        #[cfg(not(debug_assertions))]
        {
            assert_eq!(4, core::mem::size_of::<Date>());
        }
    }

    #[cfg(not(miri))]
    quickcheck::quickcheck! {
        fn prop_checked_add_then_sub(
            d1: Date,
            span: Span
        ) -> quickcheck::TestResult {
            // Force our span to have no units greater than days.
            let span = if span.largest_unit() <= Unit::Day {
                span
            } else {
                let round = SpanRound::new().largest(Unit::Day).relative(d1);
                let Ok(span) = span.round(round) else {
                    return quickcheck::TestResult::discard();
                };
                span
            };
            let Ok(d2) = d1.checked_add(span) else {
                return quickcheck::TestResult::discard();
            };
            let got = d2.checked_sub(span).unwrap();
            quickcheck::TestResult::from_bool(d1 == got)
        }

        fn prop_checked_sub_then_add(
            d1: Date,
            span: Span
        ) -> quickcheck::TestResult {
            // Force our span to have no units greater than days.
            let span = if span.largest_unit() <= Unit::Day {
                span
            } else {
                let round = SpanRound::new().largest(Unit::Day).relative(d1);
                let Ok(span) = span.round(round) else {
                    return quickcheck::TestResult::discard();
                };
                span
            };
            let Ok(d2) = d1.checked_sub(span) else {
                return quickcheck::TestResult::discard();
            };
            let got = d2.checked_add(span).unwrap();
            quickcheck::TestResult::from_bool(d1 == got)
        }

        fn prop_since_then_add(d1: Date, d2: Date) -> bool {
            let span = d1.since(d2).unwrap();
            let got = d2.checked_add(span).unwrap();
            d1 == got
        }

        fn prop_until_then_sub(d1: Date, d2: Date) -> bool {
            let span = d1.until(d2).unwrap();
            let got = d2.checked_sub(span).unwrap();
            d1 == got
        }
    }

    /// # `serde` deserializer compatibility test
    ///
    /// Serde YAML used to be unable to deserialize `jiff` types,
    /// as deserializing from bytes is not supported by the deserializer.
    ///
    /// - <https://github.com/BurntSushi/jiff/issues/138>
    /// - <https://github.com/BurntSushi/jiff/discussions/148>
    #[test]
    fn civil_date_deserialize_yaml() {
        let expected = date(2024, 10, 31);

        let deserialized: Date = serde_yaml::from_str("2024-10-31").unwrap();

        assert_eq!(deserialized, expected);

        let deserialized: Date =
            serde_yaml::from_slice("2024-10-31".as_bytes()).unwrap();

        assert_eq!(deserialized, expected);

        let cursor = Cursor::new(b"2024-10-31");
        let deserialized: Date = serde_yaml::from_reader(cursor).unwrap();

        assert_eq!(deserialized, expected);
    }

    /// Regression test where converting to `IDate` and back to do the
    /// calculation was FUBAR.
    #[test]
    fn nth_weekday_of_month() {
        let d1 = date(1998, 1, 1);
        let d2 = d1.nth_weekday_of_month(5, Weekday::Saturday).unwrap();
        assert_eq!(d2, date(1998, 1, 31));
    }
}
