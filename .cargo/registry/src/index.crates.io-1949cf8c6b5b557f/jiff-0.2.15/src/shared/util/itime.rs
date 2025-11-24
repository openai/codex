/*!
This module defines the internal core time data types.

This includes physical time (i.e., a timestamp) and civil time.

These types exist to provide a home for the core algorithms in a datetime
crate. For example, converting from a timestamp to a Gregorian calendar date
and clock time.

These routines are specifically implemented on simple primitive integer types
and implicitly assume that the inputs are valid (i.e., within Jiff's minimum
and maximum ranges).

These exist to provide `const` capabilities, and also to provide a small
reusable core of important algorithms that can be shared between `jiff` and
`jiff-static`.

# Naming

The types in this module are prefixed with letter `I` to make it clear that
they are internal types. Specifically, to distinguish them from Jiff's public
types. For example, `Date` versus `IDate`.
*/

use super::error::{err, Error};

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ITimestamp {
    pub(crate) second: i64,
    pub(crate) nanosecond: i32,
}

impl ITimestamp {
    const MIN: ITimestamp =
        ITimestamp { second: -377705023201, nanosecond: 0 };
    const MAX: ITimestamp =
        ITimestamp { second: 253402207200, nanosecond: 999_999_999 };

    /// Creates an `ITimestamp` from a Unix timestamp in seconds.
    #[inline]
    pub(crate) const fn from_second(second: i64) -> ITimestamp {
        ITimestamp { second, nanosecond: 0 }
    }

    /// Converts a Unix timestamp with an offset to a Gregorian datetime.
    ///
    /// The offset should correspond to the number of seconds required to
    /// add to this timestamp to get the local time.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn to_datetime(&self, offset: IOffset) -> IDateTime {
        let ITimestamp { mut second, mut nanosecond } = *self;
        second += offset.second as i64;
        let mut epoch_day = second.div_euclid(86_400) as i32;
        second = second.rem_euclid(86_400);
        if nanosecond < 0 {
            if second > 0 {
                second -= 1;
                nanosecond += 1_000_000_000;
            } else {
                epoch_day -= 1;
                second += 86_399;
                nanosecond += 1_000_000_000;
            }
        }

        let date = IEpochDay { epoch_day }.to_date();
        let mut time = ITimeSecond { second: second as i32 }.to_time();
        time.subsec_nanosecond = nanosecond;
        IDateTime { date, time }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct IOffset {
    pub(crate) second: i32,
}

impl IOffset {
    pub(crate) const UTC: IOffset = IOffset { second: 0 };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct IDateTime {
    pub(crate) date: IDate,
    pub(crate) time: ITime,
}

impl IDateTime {
    const MIN: IDateTime = IDateTime { date: IDate::MIN, time: ITime::MIN };
    const MAX: IDateTime = IDateTime { date: IDate::MAX, time: ITime::MAX };

    /// Converts a Gregorian datetime and its offset to a Unix timestamp.
    ///
    /// The offset should correspond to the number of seconds required to
    /// subtract from this datetime in order to get to UTC.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn to_timestamp(&self, offset: IOffset) -> ITimestamp {
        let epoch_day = self.date.to_epoch_day().epoch_day;
        let mut second = (epoch_day as i64) * 86_400
            + (self.time.to_second().second as i64);
        let mut nanosecond = self.time.subsec_nanosecond;
        second -= offset.second as i64;
        if epoch_day < 0 && nanosecond != 0 {
            second += 1;
            nanosecond -= 1_000_000_000;
        }
        ITimestamp { second, nanosecond }
    }

    /// Converts a Gregorian datetime and its offset to a Unix timestamp.
    ///
    /// If the timestamp would overflow Jiff's timestamp range, then this
    /// returns `None`.
    ///
    /// The offset should correspond to the number of seconds required to
    /// subtract from this datetime in order to get to UTC.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn to_timestamp_checked(
        &self,
        offset: IOffset,
    ) -> Option<ITimestamp> {
        let ts = self.to_timestamp(offset);
        if !(ITimestamp::MIN <= ts && ts <= ITimestamp::MAX) {
            return None;
        }
        Some(ts)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn saturating_add_seconds(&self, seconds: i32) -> IDateTime {
        self.checked_add_seconds(seconds).unwrap_or_else(|_| {
            if seconds < 0 {
                IDateTime::MIN
            } else {
                IDateTime::MAX
            }
        })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn checked_add_seconds(
        &self,
        seconds: i32,
    ) -> Result<IDateTime, Error> {
        let day_second =
            self.time.to_second().second.checked_add(seconds).ok_or_else(
                || err!("adding `{seconds}s` to datetime overflowed"),
            )?;
        let days = day_second.div_euclid(86400);
        let second = day_second.rem_euclid(86400);
        let date = self.date.checked_add_days(days)?;
        let time = ITimeSecond { second }.to_time();
        Ok(IDateTime { date, time })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct IEpochDay {
    pub(crate) epoch_day: i32,
}

impl IEpochDay {
    const MIN: IEpochDay = IEpochDay { epoch_day: -4371587 };
    const MAX: IEpochDay = IEpochDay { epoch_day: 2932896 };

    /// Converts days since the Unix epoch to a Gregorian date.
    ///
    /// This is Neri-Schneider. There's no branching or divisions.
    ///
    /// Ref: <https://github.com/cassioneri/eaf/blob/684d3cc32d14eee371d0abe4f683d6d6a49ed5c1/algorithms/neri_schneider.hpp#L40C3-L40C34>
    #[cfg_attr(feature = "perf-inline", inline(always))]
    #[allow(non_upper_case_globals, non_snake_case)] // to mimic source
    pub(crate) const fn to_date(&self) -> IDate {
        const s: u32 = 82;
        const K: u32 = 719468 + 146097 * s;
        const L: u32 = 400 * s;

        let N_U = self.epoch_day as u32;
        let N = N_U.wrapping_add(K);

        let N_1 = 4 * N + 3;
        let C = N_1 / 146097;
        let N_C = (N_1 % 146097) / 4;

        let N_2 = 4 * N_C + 3;
        let P_2 = 2939745 * (N_2 as u64);
        let Z = (P_2 / 4294967296) as u32;
        let N_Y = (P_2 % 4294967296) as u32 / 2939745 / 4;
        let Y = 100 * C + Z;

        let N_3 = 2141 * N_Y + 197913;
        let M = N_3 / 65536;
        let D = (N_3 % 65536) / 2141;

        let J = N_Y >= 306;
        let year = Y.wrapping_sub(L).wrapping_add(J as u32) as i16;
        let month = (if J { M - 12 } else { M }) as i8;
        let day = (D + 1) as i8;
        IDate { year, month, day }
    }

    /// Returns the day of the week for this epoch day.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn weekday(&self) -> IWeekday {
        // Based on Hinnant's approach here, although we use ISO weekday
        // numbering by default. Basically, this works by using the knowledge
        // that 1970-01-01 was a Thursday.
        //
        // Ref: http://howardhinnant.github.io/date_algorithms.html
        IWeekday::from_monday_zero_offset(
            (self.epoch_day + 3).rem_euclid(7) as i8
        )
    }

    /// Add the given number of days to this epoch day.
    ///
    /// If this would overflow an `i32` or result in an out-of-bounds epoch
    /// day, then this returns an error.
    #[inline]
    pub(crate) fn checked_add(&self, amount: i32) -> Result<IEpochDay, Error> {
        let epoch_day = self.epoch_day;
        let sum = epoch_day.checked_add(amount).ok_or_else(|| {
            err!("adding `{amount}` to epoch day `{epoch_day}` overflowed i32")
        })?;
        let ret = IEpochDay { epoch_day: sum };
        if !(IEpochDay::MIN <= ret && ret <= IEpochDay::MAX) {
            return Err(err!(
                "adding `{amount}` to epoch day `{epoch_day}` \
                 resulted in `{sum}`, which is not in the required \
                 epoch day range of `{min}..={max}`",
                min = IEpochDay::MIN.epoch_day,
                max = IEpochDay::MAX.epoch_day,
            ));
        }
        Ok(ret)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct IDate {
    pub(crate) year: i16,
    pub(crate) month: i8,
    pub(crate) day: i8,
}

impl IDate {
    const MIN: IDate = IDate { year: -9999, month: 1, day: 1 };
    const MAX: IDate = IDate { year: 9999, month: 12, day: 31 };

    /// Fallibly builds a new date.
    ///
    /// This checks that the given day is valid for the given year/month.
    ///
    /// No other conditions are checked. This assumes `year` and `month` are
    /// valid, and that `day >= 1`.
    #[inline]
    pub(crate) fn try_new(
        year: i16,
        month: i8,
        day: i8,
    ) -> Result<IDate, Error> {
        if day > 28 {
            let max_day = days_in_month(year, month);
            if day > max_day {
                return Err(err!(
                    "day={day} is out of range for year={year} \
                     and month={month}, must be in range 1..={max_day}",
                ));
            }
        }
        Ok(IDate { year, month, day })
    }

    /// Returns the date corresponding to the day of the given year. The day
    /// of the year should be a value in `1..=366`, with `366` only being valid
    /// if `year` is a leap year.
    ///
    /// This assumes that `year` is valid, but returns an error if `day` is
    /// not in the range `1..=366`.
    #[inline]
    pub(crate) fn from_day_of_year(
        year: i16,
        day: i16,
    ) -> Result<IDate, Error> {
        if !(1 <= day && day <= 366) {
            return Err(err!(
                "day-of-year={day} is out of range for year={year}, \
                 must be in range 1..={max_day}",
                max_day = days_in_year(year),
            ));
        }
        let start = IDate { year, month: 1, day: 1 }.to_epoch_day();
        let end = start
            .checked_add(i32::from(day) - 1)
            .map_err(|_| {
                err!(
                    "failed to find date for \
                     year={year} and day-of-year={day}: \
                     adding `{day}` to `{start}` overflows \
                     Jiff's range",
                    start = start.epoch_day,
                )
            })?
            .to_date();
        // If we overflowed into the next year, then `day` is too big.
        if year != end.year {
            // Can only happen given day=366 and this is a leap year.
            debug_assert_eq!(day, 366);
            debug_assert!(!is_leap_year(year));
            return Err(err!(
                "day-of-year={day} is out of range for year={year}, \
                 must be in range 1..={max_day}",
                max_day = days_in_year(year),
            ));
        }
        Ok(end)
    }

    /// Returns the date corresponding to the day of the given year. The day
    /// of the year should be a value in `1..=365`, with February 29 being
    /// completely ignored. That is, it is guaranteed that Febraury 29 will
    /// never be returned by this function. It is impossible.
    ///
    /// This assumes that `year` is valid, but returns an error if `day` is
    /// not in the range `1..=365`.
    #[inline]
    pub(crate) fn from_day_of_year_no_leap(
        year: i16,
        mut day: i16,
    ) -> Result<IDate, Error> {
        if !(1 <= day && day <= 365) {
            return Err(err!(
                "day-of-year={day} is out of range for year={year}, \
                 must be in range 1..=365",
            ));
        }
        if day >= 60 && is_leap_year(year) {
            day += 1;
        }
        // The boundary check above guarantees this always succeeds.
        Ok(IDate::from_day_of_year(year, day).unwrap())
    }

    /// Converts a Gregorian date to days since the Unix epoch.
    ///
    /// This is Neri-Schneider. There's no branching or divisions.
    ///
    /// Ref: https://github.com/cassioneri/eaf/blob/684d3cc32d14eee371d0abe4f683d6d6a49ed5c1/algorithms/neri_schneider.hpp#L83
    #[cfg_attr(feature = "perf-inline", inline(always))]
    #[allow(non_upper_case_globals, non_snake_case)] // to mimic source
    pub(crate) const fn to_epoch_day(&self) -> IEpochDay {
        const s: u32 = 82;
        const K: u32 = 719468 + 146097 * s;
        const L: u32 = 400 * s;

        let year = self.year as u32;
        let month = self.month as u32;
        let day = self.day as u32;

        let J = month <= 2;
        let Y = year.wrapping_add(L).wrapping_sub(J as u32);
        let M = if J { month + 12 } else { month };
        let D = day - 1;
        let C = Y / 100;

        let y_star = 1461 * Y / 4 - C + C / 4;
        let m_star = (979 * M - 2919) / 32;
        let N = y_star + m_star + D;

        let N_U = N.wrapping_sub(K);
        let epoch_day = N_U as i32;
        IEpochDay { epoch_day }
    }

    /// Returns the day of the week for this date.
    #[inline]
    pub(crate) const fn weekday(&self) -> IWeekday {
        self.to_epoch_day().weekday()
    }

    /// Returns the `nth` weekday of the month represented by this date.
    ///
    /// `nth` must be non-zero and otherwise in the range `-5..=5`. If it
    /// isn't, an error is returned.
    ///
    /// This also returns an error if `abs(nth)==5` and there is no "5th"
    /// weekday of this month.
    #[inline]
    pub(crate) fn nth_weekday_of_month(
        &self,
        nth: i8,
        weekday: IWeekday,
    ) -> Result<IDate, Error> {
        if nth == 0 || !(-5 <= nth && nth <= 5) {
            return Err(err!(
                "got nth weekday of `{nth}`, but \
                 must be non-zero and in range `-5..=5`",
            ));
        }
        if nth > 0 {
            let first_weekday = self.first_of_month().weekday();
            let diff = weekday.since(first_weekday);
            let day = diff + 1 + (nth - 1) * 7;
            IDate::try_new(self.year, self.month, day)
        } else {
            let last = self.last_of_month();
            let last_weekday = last.weekday();
            let diff = last_weekday.since(weekday);
            let day = last.day - diff - (nth.abs() - 1) * 7;
            // Our math can go below 1 when nth is -5 and there is no "5th from
            // last" weekday in this month. Since this is outside the bounds
            // of `Day`, we can't let this boundary condition escape. So we
            // check it here.
            if day < 1 {
                return Err(err!(
                    "day={day} is out of range for year={year} \
                     and month={month}, must be in range 1..={max_day}",
                    year = self.year,
                    month = self.month,
                    max_day = days_in_month(self.year, self.month),
                ));
            }
            IDate::try_new(self.year, self.month, day)
        }
    }

    /// Returns the day before this date.
    #[inline]
    pub(crate) fn yesterday(self) -> Result<IDate, Error> {
        if self.day == 1 {
            if self.month == 1 {
                let year = self.year - 1;
                if year <= -10000 {
                    return Err(err!(
                        "returning yesterday for -9999-01-01 is not \
                         possible because it is less than Jiff's supported
                         minimum date",
                    ));
                }
                return Ok(IDate { year, month: 12, day: 31 });
            }
            let month = self.month - 1;
            let day = days_in_month(self.year, month);
            return Ok(IDate { month, day, ..self });
        }
        Ok(IDate { day: self.day - 1, ..self })
    }

    /// Returns the day after this date.
    #[inline]
    pub(crate) fn tomorrow(self) -> Result<IDate, Error> {
        if self.day >= 28 && self.day == days_in_month(self.year, self.month) {
            if self.month == 12 {
                let year = self.year + 1;
                if year >= 10000 {
                    return Err(err!(
                        "returning tomorrow for 9999-12-31 is not \
                         possible because it is greater than Jiff's supported
                         maximum date",
                    ));
                }
                return Ok(IDate { year, month: 1, day: 1 });
            }
            let month = self.month + 1;
            return Ok(IDate { month, day: 1, ..self });
        }
        Ok(IDate { day: self.day + 1, ..self })
    }

    /// Returns the year one year before this date.
    #[inline]
    pub(crate) fn prev_year(self) -> Result<i16, Error> {
        let year = self.year - 1;
        if year <= -10_000 {
            return Err(err!(
                "returning previous year for {year:04}-{month:02}-{day:02} is \
                 not possible because it is less than Jiff's supported \
                 minimum date",
                year = self.year,
                month = self.month,
                day = self.day,
            ));
        }
        Ok(year)
    }

    /// Returns the year one year from this date.
    #[inline]
    pub(crate) fn next_year(self) -> Result<i16, Error> {
        let year = self.year + 1;
        if year >= 10_000 {
            return Err(err!(
                "returning next year for {year:04}-{month:02}-{day:02} is \
                 not possible because it is greater than Jiff's supported \
                 maximum date",
                year = self.year,
                month = self.month,
                day = self.day,
            ));
        }
        Ok(year)
    }

    /// Add the number of days to this date.
    #[inline]
    pub(crate) fn checked_add_days(
        &self,
        amount: i32,
    ) -> Result<IDate, Error> {
        match amount {
            0 => Ok(*self),
            -1 => self.yesterday(),
            1 => self.tomorrow(),
            n => self.to_epoch_day().checked_add(n).map(|d| d.to_date()),
        }
    }

    #[inline]
    fn first_of_month(&self) -> IDate {
        IDate { day: 1, ..*self }
    }

    #[inline]
    fn last_of_month(&self) -> IDate {
        IDate { day: days_in_month(self.year, self.month), ..*self }
    }

    #[cfg(test)]
    pub(crate) fn at(
        &self,
        hour: i8,
        minute: i8,
        second: i8,
        subsec_nanosecond: i32,
    ) -> IDateTime {
        let time = ITime { hour, minute, second, subsec_nanosecond };
        IDateTime { date: *self, time }
    }
}

/// Represents a clock time.
///
/// This uses units of hours, minutes, seconds and fractional seconds (to
/// nanosecond precision).
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ITime {
    pub(crate) hour: i8,
    pub(crate) minute: i8,
    pub(crate) second: i8,
    pub(crate) subsec_nanosecond: i32,
}

impl ITime {
    pub(crate) const ZERO: ITime =
        ITime { hour: 0, minute: 0, second: 0, subsec_nanosecond: 0 };
    pub(crate) const MIN: ITime =
        ITime { hour: 0, minute: 0, second: 0, subsec_nanosecond: 0 };
    pub(crate) const MAX: ITime = ITime {
        hour: 23,
        minute: 59,
        second: 59,
        subsec_nanosecond: 999_999_999,
    };

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn to_second(&self) -> ITimeSecond {
        let mut second: i32 = 0;
        second += (self.hour as i32) * 3600;
        second += (self.minute as i32) * 60;
        second += self.second as i32;
        ITimeSecond { second }
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn to_nanosecond(&self) -> ITimeNanosecond {
        let mut nanosecond: i64 = 0;
        nanosecond += (self.hour as i64) * 3_600_000_000_000;
        nanosecond += (self.minute as i64) * 60_000_000_000;
        nanosecond += (self.second as i64) * 1_000_000_000;
        nanosecond += self.subsec_nanosecond as i64;
        ITimeNanosecond { nanosecond }
    }
}

/// Represents a single point in the day, to second precision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ITimeSecond {
    pub(crate) second: i32,
}

impl ITimeSecond {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn to_time(&self) -> ITime {
        let mut second = self.second;
        let mut time = ITime::ZERO;
        if second != 0 {
            time.hour = (second / 3600) as i8;
            second %= 3600;
            if second != 0 {
                time.minute = (second / 60) as i8;
                time.second = (second % 60) as i8;
            }
        }
        time
    }
}

/// Represents a single point in the day, to nanosecond precision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ITimeNanosecond {
    pub(crate) nanosecond: i64,
}

impl ITimeNanosecond {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) const fn to_time(&self) -> ITime {
        let mut nanosecond = self.nanosecond;
        let mut time = ITime::ZERO;
        if nanosecond != 0 {
            time.hour = (nanosecond / 3_600_000_000_000) as i8;
            nanosecond %= 3_600_000_000_000;
            if nanosecond != 0 {
                time.minute = (nanosecond / 60_000_000_000) as i8;
                nanosecond %= 60_000_000_000;
                if nanosecond != 0 {
                    time.second = (nanosecond / 1_000_000_000) as i8;
                    time.subsec_nanosecond =
                        (nanosecond % 1_000_000_000) as i32;
                }
            }
        }
        time
    }
}

/// Represents a weekday.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct IWeekday {
    /// Range is `1..=6` with `1=Monday`.
    offset: i8,
}

impl IWeekday {
    /// Creates a weekday assuming the week starts on Monday and Monday is at
    /// offset `0`.
    #[inline]
    pub(crate) const fn from_monday_zero_offset(offset: i8) -> IWeekday {
        assert!(0 <= offset && offset <= 6);
        IWeekday::from_monday_one_offset(offset + 1)
    }

    /// Creates a weekday assuming the week starts on Monday and Monday is at
    /// offset `1`.
    #[inline]
    pub(crate) const fn from_monday_one_offset(offset: i8) -> IWeekday {
        assert!(1 <= offset && offset <= 7);
        IWeekday { offset }
    }

    /// Creates a weekday assuming the week starts on Sunday and Sunday is at
    /// offset `0`.
    #[inline]
    pub(crate) const fn from_sunday_zero_offset(offset: i8) -> IWeekday {
        assert!(0 <= offset && offset <= 6);
        IWeekday::from_monday_zero_offset((offset - 1).rem_euclid(7))
    }

    /// Creates a weekday assuming the week starts on Sunday and Sunday is at
    /// offset `1`.
    #[cfg(test)] // currently dead code
    #[inline]
    pub(crate) const fn from_sunday_one_offset(offset: i8) -> IWeekday {
        assert!(1 <= offset && offset <= 7);
        IWeekday::from_sunday_zero_offset(offset - 1)
    }

    /// Returns this weekday as an offset in the range `0..=6` where
    /// `0=Monday`.
    #[inline]
    pub(crate) const fn to_monday_zero_offset(self) -> i8 {
        self.to_monday_one_offset() - 1
    }

    /// Returns this weekday as an offset in the range `1..=7` where
    /// `1=Monday`.
    #[inline]
    pub(crate) const fn to_monday_one_offset(self) -> i8 {
        self.offset
    }

    /// Returns this weekday as an offset in the range `0..=6` where
    /// `0=Sunday`.
    #[cfg(test)] // currently dead code
    #[inline]
    pub(crate) const fn to_sunday_zero_offset(self) -> i8 {
        (self.to_monday_zero_offset() + 1) % 7
    }

    /// Returns this weekday as an offset in the range `1..=7` where
    /// `1=Sunday`.
    #[cfg(test)] // currently dead code
    #[inline]
    pub(crate) const fn to_sunday_one_offset(self) -> i8 {
        self.to_sunday_zero_offset() + 1
    }

    #[inline]
    pub(crate) const fn since(self, other: IWeekday) -> i8 {
        (self.to_monday_zero_offset() - other.to_monday_zero_offset())
            .rem_euclid(7)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IAmbiguousOffset {
    Unambiguous { offset: IOffset },
    Gap { before: IOffset, after: IOffset },
    Fold { before: IOffset, after: IOffset },
}

/// Returns true if and only if the given year is a leap year.
///
/// A leap year is a year with 366 days. Typical years have 365 days.
#[inline]
pub(crate) const fn is_leap_year(year: i16) -> bool {
    // From: https://github.com/BurntSushi/jiff/pull/23
    let d = if year % 25 != 0 { 4 } else { 16 };
    (year % d) == 0
}

/// Return the number of days in the given year.
#[inline]
pub(crate) const fn days_in_year(year: i16) -> i16 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

/// Return the number of days in the given month.
#[inline]
pub(crate) const fn days_in_month(year: i16, month: i8) -> i8 {
    // From: https://github.com/BurntSushi/jiff/pull/23
    if month == 2 {
        if is_leap_year(year) {
            29
        } else {
            28
        }
    } else {
        30 | (month ^ month >> 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_epochday_date() {
        for year in -9999..=9999 {
            for month in 1..=12 {
                for day in 1..=days_in_month(year, month) {
                    let date = IDate { year, month, day };
                    let epoch_day = date.to_epoch_day();
                    let date_roundtrip = epoch_day.to_date();
                    assert_eq!(date, date_roundtrip);
                }
            }
        }
    }

    #[test]
    fn roundtrip_second_time() {
        for second in 0..=86_399 {
            let second = ITimeSecond { second };
            let time = second.to_time();
            let second_roundtrip = time.to_second();
            assert_eq!(second, second_roundtrip);
        }
    }

    #[test]
    fn roundtrip_nanosecond_time() {
        for second in 0..=86_399 {
            for nanosecond in
                [0, 250_000_000, 500_000_000, 750_000_000, 900_000_000]
            {
                let nanosecond = ITimeNanosecond {
                    nanosecond: (second * 1_000_000_000 + nanosecond),
                };
                let time = nanosecond.to_time();
                let nanosecond_roundtrip = time.to_nanosecond();
                assert_eq!(nanosecond, nanosecond_roundtrip);
            }
        }
    }

    #[test]
    fn nth_weekday() {
        let d1 = IDate { year: 2017, month: 3, day: 1 };
        let wday = IWeekday::from_sunday_zero_offset(5);
        let d2 = d1.nth_weekday_of_month(2, wday).unwrap();
        assert_eq!(d2, IDate { year: 2017, month: 3, day: 10 });

        let d1 = IDate { year: 2024, month: 3, day: 1 };
        let wday = IWeekday::from_sunday_zero_offset(4);
        let d2 = d1.nth_weekday_of_month(-1, wday).unwrap();
        assert_eq!(d2, IDate { year: 2024, month: 3, day: 28 });

        let d1 = IDate { year: 2024, month: 3, day: 25 };
        let wday = IWeekday::from_sunday_zero_offset(1);
        assert!(d1.nth_weekday_of_month(5, wday).is_err());
        assert!(d1.nth_weekday_of_month(-5, wday).is_err());

        let d1 = IDate { year: 1998, month: 1, day: 1 };
        let wday = IWeekday::from_sunday_zero_offset(6);
        let d2 = d1.nth_weekday_of_month(5, wday).unwrap();
        assert_eq!(d2, IDate { year: 1998, month: 1, day: 31 });
    }

    #[test]
    fn weekday() {
        let wday = IWeekday::from_sunday_zero_offset(0);
        assert_eq!(wday.to_monday_one_offset(), 7);

        let wday = IWeekday::from_monday_one_offset(7);
        assert_eq!(wday.to_sunday_zero_offset(), 0);

        let wday = IWeekday::from_sunday_one_offset(1);
        assert_eq!(wday.to_monday_zero_offset(), 6);

        let wday = IWeekday::from_monday_zero_offset(6);
        assert_eq!(wday.to_sunday_one_offset(), 1);
    }

    #[test]
    fn weekday_since() {
        let wday1 = IWeekday::from_sunday_zero_offset(0);
        let wday2 = IWeekday::from_sunday_zero_offset(6);
        assert_eq!(wday2.since(wday1), 6);
        assert_eq!(wday1.since(wday2), 1);
    }

    #[test]
    fn leap_year() {
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(2001));
        assert!(!is_leap_year(2002));
        assert!(!is_leap_year(2003));
        assert!(is_leap_year(2004));
    }

    #[test]
    fn number_of_days_in_month() {
        assert_eq!(days_in_month(2024, 1), 31);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2024, 3), 31);
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 5), 31);
        assert_eq!(days_in_month(2024, 6), 30);
        assert_eq!(days_in_month(2024, 7), 31);
        assert_eq!(days_in_month(2024, 8), 31);
        assert_eq!(days_in_month(2024, 9), 30);
        assert_eq!(days_in_month(2024, 10), 31);
        assert_eq!(days_in_month(2024, 11), 30);
        assert_eq!(days_in_month(2024, 12), 31);

        assert_eq!(days_in_month(2025, 1), 31);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2025, 3), 31);
        assert_eq!(days_in_month(2025, 4), 30);
        assert_eq!(days_in_month(2025, 5), 31);
        assert_eq!(days_in_month(2025, 6), 30);
        assert_eq!(days_in_month(2025, 7), 31);
        assert_eq!(days_in_month(2025, 8), 31);
        assert_eq!(days_in_month(2025, 9), 30);
        assert_eq!(days_in_month(2025, 10), 31);
        assert_eq!(days_in_month(2025, 11), 30);
        assert_eq!(days_in_month(2025, 12), 31);

        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
    }

    #[test]
    fn yesterday() {
        let d1 = IDate { year: 2025, month: 4, day: 7 };
        let d2 = d1.yesterday().unwrap();
        assert_eq!(d2, IDate { year: 2025, month: 4, day: 6 });

        let d1 = IDate { year: 2025, month: 4, day: 1 };
        let d2 = d1.yesterday().unwrap();
        assert_eq!(d2, IDate { year: 2025, month: 3, day: 31 });

        let d1 = IDate { year: 2025, month: 1, day: 1 };
        let d2 = d1.yesterday().unwrap();
        assert_eq!(d2, IDate { year: 2024, month: 12, day: 31 });

        let d1 = IDate { year: -9999, month: 1, day: 1 };
        assert_eq!(d1.yesterday().ok(), None);
    }

    #[test]
    fn tomorrow() {
        let d1 = IDate { year: 2025, month: 4, day: 7 };
        let d2 = d1.tomorrow().unwrap();
        assert_eq!(d2, IDate { year: 2025, month: 4, day: 8 });

        let d1 = IDate { year: 2025, month: 3, day: 31 };
        let d2 = d1.tomorrow().unwrap();
        assert_eq!(d2, IDate { year: 2025, month: 4, day: 1 });

        let d1 = IDate { year: 2025, month: 12, day: 31 };
        let d2 = d1.tomorrow().unwrap();
        assert_eq!(d2, IDate { year: 2026, month: 1, day: 1 });

        let d1 = IDate { year: 9999, month: 12, day: 31 };
        assert_eq!(d1.tomorrow().ok(), None);
    }
}
