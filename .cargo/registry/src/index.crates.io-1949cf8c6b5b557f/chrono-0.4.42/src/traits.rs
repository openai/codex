use crate::{IsoWeek, Month, Weekday};

/// The common set of methods for date component.
///
/// Methods such as [`year`], [`month`], [`day`] and [`weekday`] can be used to get basic
/// information about the date.
///
/// The `with_*` methods can change the date.
///
/// # Warning
///
/// The `with_*` methods can be convenient to change a single component of a date, but they must be
/// used with some care. Examples to watch out for:
///
/// - [`with_year`] changes the year component of a year-month-day value. Don't use this method if
///   you want the ordinal to stay the same after changing the year, of if you want the week and
///   weekday values to stay the same.
/// - Don't combine two `with_*` methods to change two components of the date. For example to
///   change both the year and month components of a date. This could fail because an intermediate
///   value does not exist, while the final date would be valid.
///
/// For more complex changes to a date, it is best to use the methods on [`NaiveDate`] to create a
/// new value instead of altering an existing date.
///
/// [`year`]: Datelike::year
/// [`month`]: Datelike::month
/// [`day`]: Datelike::day
/// [`weekday`]: Datelike::weekday
/// [`with_year`]: Datelike::with_year
/// [`NaiveDate`]: crate::NaiveDate
pub trait Datelike: Sized {
    /// Returns the year number in the [calendar date](./naive/struct.NaiveDate.html#calendar-date).
    fn year(&self) -> i32;

    /// Returns the absolute year number starting from 1 with a boolean flag,
    /// which is false when the year predates the epoch (BCE/BC) and true otherwise (CE/AD).
    #[inline]
    fn year_ce(&self) -> (bool, u32) {
        let year = self.year();
        if year < 1 { (false, (1 - year) as u32) } else { (true, year as u32) }
    }

    /// Returns the quarter number starting from 1.
    ///
    /// The return value ranges from 1 to 4.
    #[inline]
    fn quarter(&self) -> u32 {
        (self.month() - 1).div_euclid(3) + 1
    }

    /// Returns the month number starting from 1.
    ///
    /// The return value ranges from 1 to 12.
    fn month(&self) -> u32;

    /// Returns the month number starting from 0.
    ///
    /// The return value ranges from 0 to 11.
    fn month0(&self) -> u32;

    /// Returns the day of month starting from 1.
    ///
    /// The return value ranges from 1 to 31. (The last day of month differs by months.)
    fn day(&self) -> u32;

    /// Returns the day of month starting from 0.
    ///
    /// The return value ranges from 0 to 30. (The last day of month differs by months.)
    fn day0(&self) -> u32;

    /// Returns the day of year starting from 1.
    ///
    /// The return value ranges from 1 to 366. (The last day of year differs by years.)
    fn ordinal(&self) -> u32;

    /// Returns the day of year starting from 0.
    ///
    /// The return value ranges from 0 to 365. (The last day of year differs by years.)
    fn ordinal0(&self) -> u32;

    /// Returns the day of week.
    fn weekday(&self) -> Weekday;

    /// Returns the ISO week.
    fn iso_week(&self) -> IsoWeek;

    /// Makes a new value with the year number changed, while keeping the same month and day.
    ///
    /// This method assumes you want to work on the date as a year-month-day value. Don't use it if
    /// you want the ordinal to stay the same after changing the year, of if you want the week and
    /// weekday values to stay the same.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (February 29 in a non-leap year).
    /// - The year is out of range for [`NaiveDate`].
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    ///
    /// [`NaiveDate`]: crate::NaiveDate
    /// [`DateTime<Tz>`]: crate::DateTime
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2020, 5, 13).unwrap().with_year(2023).unwrap(),
    ///     NaiveDate::from_ymd_opt(2023, 5, 13).unwrap()
    /// );
    /// // Resulting date 2023-02-29 does not exist:
    /// assert!(NaiveDate::from_ymd_opt(2020, 2, 29).unwrap().with_year(2023).is_none());
    ///
    /// // Don't use `with_year` if you want the ordinal date to stay the same:
    /// assert_ne!(
    ///     NaiveDate::from_yo_opt(2020, 100).unwrap().with_year(2023).unwrap(),
    ///     NaiveDate::from_yo_opt(2023, 100).unwrap() // result is 2023-101
    /// );
    /// ```
    fn with_year(&self, year: i32) -> Option<Self>;

    /// Makes a new value with the month number (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (for example `month(4)` when day of the month is 31).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `month` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2023, 5, 12).unwrap().with_month(9).unwrap(),
    ///     NaiveDate::from_ymd_opt(2023, 9, 12).unwrap()
    /// );
    /// // Resulting date 2023-09-31 does not exist:
    /// assert!(NaiveDate::from_ymd_opt(2023, 5, 31).unwrap().with_month(9).is_none());
    /// ```
    ///
    /// Don't combine multiple `Datelike::with_*` methods. The intermediate value may not exist.
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// fn with_year_month(date: NaiveDate, year: i32, month: u32) -> Option<NaiveDate> {
    ///     date.with_year(year)?.with_month(month)
    /// }
    /// let d = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
    /// assert!(with_year_month(d, 2019, 1).is_none()); // fails because of invalid intermediate value
    ///
    /// // Correct version:
    /// fn with_year_month_fixed(date: NaiveDate, year: i32, month: u32) -> Option<NaiveDate> {
    ///     NaiveDate::from_ymd_opt(year, month, date.day())
    /// }
    /// let d = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
    /// assert_eq!(with_year_month_fixed(d, 2019, 1), NaiveDate::from_ymd_opt(2019, 1, 29));
    /// ```
    fn with_month(&self, month: u32) -> Option<Self>;

    /// Makes a new value with the month number (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (for example `month0(3)` when day of the month is 31).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `month0` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    fn with_month0(&self, month0: u32) -> Option<Self>;

    /// Makes a new value with the day of month (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (for example `day(31)` in April).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `day` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    fn with_day(&self, day: u32) -> Option<Self>;

    /// Makes a new value with the day of month (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (for example `day0(30)` in April).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `day0` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    fn with_day0(&self, day0: u32) -> Option<Self>;

    /// Makes a new value with the day of year (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (`with_ordinal(366)` in a non-leap year).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `ordinal` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    fn with_ordinal(&self, ordinal: u32) -> Option<Self>;

    /// Makes a new value with the day of year (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` when:
    ///
    /// - The resulting date does not exist (`with_ordinal0(365)` in a non-leap year).
    /// - In case of [`DateTime<Tz>`] if the resulting date and time fall within a timezone
    ///   transition such as from DST to standard time.
    /// - The value for `ordinal0` is out of range.
    ///
    /// [`DateTime<Tz>`]: crate::DateTime
    fn with_ordinal0(&self, ordinal0: u32) -> Option<Self>;

    /// Counts the days in the proleptic Gregorian calendar, with January 1, Year 1 (CE) as day 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().num_days_from_ce(), 719_163);
    /// assert_eq!(NaiveDate::from_ymd_opt(2, 1, 1).unwrap().num_days_from_ce(), 366);
    /// assert_eq!(NaiveDate::from_ymd_opt(1, 1, 1).unwrap().num_days_from_ce(), 1);
    /// assert_eq!(NaiveDate::from_ymd_opt(0, 1, 1).unwrap().num_days_from_ce(), -365);
    /// ```
    fn num_days_from_ce(&self) -> i32 {
        // See test_num_days_from_ce_against_alternative_impl below for a more straightforward
        // implementation.

        // we know this wouldn't overflow since year is limited to 1/2^13 of i32's full range.
        let mut year = self.year() - 1;
        let mut ndays = 0;
        if year < 0 {
            let excess = 1 + (-year) / 400;
            year += excess * 400;
            ndays -= excess * 146_097;
        }
        let div_100 = year / 100;
        ndays += ((year * 1461) >> 2) - div_100 + (div_100 >> 2);
        ndays + self.ordinal() as i32
    }

    /// Get the length in days of the month
    fn num_days_in_month(&self) -> u8 {
        use num_traits::FromPrimitive;
        // The value returned from `self.month()` is guaranteed to be in the
        // range [1,12], which will never result in a `None` value here.
        let month = Month::from_u32(self.month()).unwrap();
        // `Month::num_days` will only return `None` if the provided year is out
        // of range. Since we are passing it directly from a verified date, we
        // know it is in range, and the result will never be `None`.
        month.num_days(self.year()).unwrap()
    }
}

/// The common set of methods for time component.
pub trait Timelike: Sized {
    /// Returns the hour number from 0 to 23.
    fn hour(&self) -> u32;

    /// Returns the hour number from 1 to 12 with a boolean flag,
    /// which is false for AM and true for PM.
    #[inline]
    fn hour12(&self) -> (bool, u32) {
        let hour = self.hour();
        let mut hour12 = hour % 12;
        if hour12 == 0 {
            hour12 = 12;
        }
        (hour >= 12, hour12)
    }

    /// Returns the minute number from 0 to 59.
    fn minute(&self) -> u32;

    /// Returns the second number from 0 to 59.
    fn second(&self) -> u32;

    /// Returns the number of nanoseconds since the whole non-leap second.
    /// The range from 1,000,000,000 to 1,999,999,999 represents
    /// the [leap second](./naive/struct.NaiveTime.html#leap-second-handling).
    fn nanosecond(&self) -> u32;

    /// Makes a new value with the hour number changed.
    ///
    /// Returns `None` when the resulting value would be invalid.
    fn with_hour(&self, hour: u32) -> Option<Self>;

    /// Makes a new value with the minute number changed.
    ///
    /// Returns `None` when the resulting value would be invalid.
    fn with_minute(&self, min: u32) -> Option<Self>;

    /// Makes a new value with the second number changed.
    ///
    /// Returns `None` when the resulting value would be invalid.
    /// As with the [`second`](#tymethod.second) method,
    /// the input range is restricted to 0 through 59.
    fn with_second(&self, sec: u32) -> Option<Self>;

    /// Makes a new value with nanoseconds since the whole non-leap second changed.
    ///
    /// Returns `None` when the resulting value would be invalid.
    /// As with the [`nanosecond`](#tymethod.nanosecond) method,
    /// the input range can exceed 1,000,000,000 for leap seconds.
    fn with_nanosecond(&self, nano: u32) -> Option<Self>;

    /// Returns the number of non-leap seconds past the last midnight.
    ///
    /// Every value in 00:00:00-23:59:59 maps to an integer in 0-86399.
    ///
    /// This method is not intended to provide the real number of seconds since midnight on a given
    /// day. It does not take things like DST transitions into account.
    #[inline]
    fn num_seconds_from_midnight(&self) -> u32 {
        self.hour() * 3600 + self.minute() * 60 + self.second()
    }
}

#[cfg(test)]
mod tests {
    use super::Datelike;
    use crate::{Days, NaiveDate};

    /// Tests `Datelike::num_days_from_ce` against an alternative implementation.
    ///
    /// The alternative implementation is not as short as the current one but it is simpler to
    /// understand, with less unexplained magic constants.
    #[test]
    fn test_num_days_from_ce_against_alternative_impl() {
        /// Returns the number of multiples of `div` in the range `start..end`.
        ///
        /// If the range `start..end` is back-to-front, i.e. `start` is greater than `end`, the
        /// behaviour is defined by the following equation:
        /// `in_between(start, end, div) == - in_between(end, start, div)`.
        ///
        /// When `div` is 1, this is equivalent to `end - start`, i.e. the length of `start..end`.
        ///
        /// # Panics
        ///
        /// Panics if `div` is not positive.
        fn in_between(start: i32, end: i32, div: i32) -> i32 {
            assert!(div > 0, "in_between: nonpositive div = {div}");
            let start = (start.div_euclid(div), start.rem_euclid(div));
            let end = (end.div_euclid(div), end.rem_euclid(div));
            // The lowest multiple of `div` greater than or equal to `start`, divided.
            let start = start.0 + (start.1 != 0) as i32;
            // The lowest multiple of `div` greater than or equal to   `end`, divided.
            let end = end.0 + (end.1 != 0) as i32;
            end - start
        }

        /// Alternative implementation to `Datelike::num_days_from_ce`
        fn num_days_from_ce<Date: Datelike>(date: &Date) -> i32 {
            let year = date.year();
            let diff = move |div| in_between(1, year, div);
            // 365 days a year, one more in leap years. In the gregorian calendar, leap years are all
            // the multiples of 4 except multiples of 100 but including multiples of 400.
            date.ordinal() as i32 + 365 * diff(1) + diff(4) - diff(100) + diff(400)
        }

        for year in NaiveDate::MIN.year()..=NaiveDate::MAX.year() {
            let jan1_year = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
            assert_eq!(
                jan1_year.num_days_from_ce(),
                num_days_from_ce(&jan1_year),
                "on {jan1_year:?}"
            );
            let mid_year = jan1_year + Days::new(133);
            assert_eq!(mid_year.num_days_from_ce(), num_days_from_ce(&mid_year), "on {mid_year:?}");
        }
    }

    #[test]
    fn test_num_days_in_month() {
        let feb_leap_year = NaiveDate::from_ymd_opt(2004, 2, 1).unwrap();
        assert_eq!(feb_leap_year.num_days_in_month(), 29);
        let feb = feb_leap_year.with_year(2005).unwrap();
        assert_eq!(feb.num_days_in_month(), 28);
        let march = feb.with_month(3).unwrap();
        assert_eq!(march.num_days_in_month(), 31);
    }
}
