// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! Functionality for rounding or truncating a `DateTime` by a `TimeDelta`.

use crate::{DateTime, NaiveDateTime, TimeDelta, TimeZone, Timelike};
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};

/// Extension trait for subsecond rounding or truncation to a maximum number
/// of digits. Rounding can be used to decrease the error variance when
/// serializing/persisting to lower precision. Truncation is the default
/// behavior in Chrono display formatting.  Either can be used to guarantee
/// equality (e.g. for testing) when round-tripping through a lower precision
/// format.
pub trait SubsecRound {
    /// Return a copy rounded to the specified number of subsecond digits. With
    /// 9 or more digits, self is returned unmodified. Halfway values are
    /// rounded up (away from zero).
    ///
    /// # Example
    /// ``` rust
    /// # use chrono::{SubsecRound, Timelike, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 11)
    ///     .unwrap()
    ///     .and_hms_milli_opt(12, 0, 0, 154)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(dt.round_subsecs(2).nanosecond(), 150_000_000);
    /// assert_eq!(dt.round_subsecs(1).nanosecond(), 200_000_000);
    /// ```
    fn round_subsecs(self, digits: u16) -> Self;

    /// Return a copy truncated to the specified number of subsecond
    /// digits. With 9 or more digits, self is returned unmodified.
    ///
    /// # Example
    /// ``` rust
    /// # use chrono::{SubsecRound, Timelike, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 11)
    ///     .unwrap()
    ///     .and_hms_milli_opt(12, 0, 0, 154)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(dt.trunc_subsecs(2).nanosecond(), 150_000_000);
    /// assert_eq!(dt.trunc_subsecs(1).nanosecond(), 100_000_000);
    /// ```
    fn trunc_subsecs(self, digits: u16) -> Self;
}

impl<T> SubsecRound for T
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    fn round_subsecs(self, digits: u16) -> T {
        let span = span_for_digits(digits);
        let delta_down = self.nanosecond() % span;
        if delta_down > 0 {
            let delta_up = span - delta_down;
            if delta_up <= delta_down {
                self + TimeDelta::nanoseconds(delta_up.into())
            } else {
                self - TimeDelta::nanoseconds(delta_down.into())
            }
        } else {
            self // unchanged
        }
    }

    fn trunc_subsecs(self, digits: u16) -> T {
        let span = span_for_digits(digits);
        let delta_down = self.nanosecond() % span;
        if delta_down > 0 {
            self - TimeDelta::nanoseconds(delta_down.into())
        } else {
            self // unchanged
        }
    }
}

// Return the maximum span in nanoseconds for the target number of digits.
const fn span_for_digits(digits: u16) -> u32 {
    // fast lookup form of: 10^(9-min(9,digits))
    match digits {
        0 => 1_000_000_000,
        1 => 100_000_000,
        2 => 10_000_000,
        3 => 1_000_000,
        4 => 100_000,
        5 => 10_000,
        6 => 1_000,
        7 => 100,
        8 => 10,
        _ => 1,
    }
}

/// Extension trait for rounding or truncating a DateTime by a TimeDelta.
///
/// # Limitations
/// Both rounding and truncating are done via [`TimeDelta::num_nanoseconds`] and
/// [`DateTime::timestamp_nanos_opt`]. This means that they will fail if either the
/// `TimeDelta` or the `DateTime` are too big to represented as nanoseconds. They
/// will also fail if the `TimeDelta` is bigger than the timestamp, negative or zero.
pub trait DurationRound: Sized {
    /// Error that can occur in rounding or truncating
    #[cfg(feature = "std")]
    type Err: std::error::Error;

    /// Error that can occur in rounding or truncating
    #[cfg(all(not(feature = "std"), feature = "core-error"))]
    type Err: core::error::Error;

    /// Error that can occur in rounding or truncating
    #[cfg(all(not(feature = "std"), not(feature = "core-error")))]
    type Err: fmt::Debug + fmt::Display;

    /// Return a copy rounded by TimeDelta.
    ///
    /// # Example
    /// ``` rust
    /// # use chrono::{DurationRound, TimeDelta, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 11)
    ///     .unwrap()
    ///     .and_hms_milli_opt(12, 0, 0, 154)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(
    ///     dt.duration_round(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
    ///     "2018-01-11 12:00:00.150 UTC"
    /// );
    /// assert_eq!(
    ///     dt.duration_round(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
    ///     "2018-01-12 00:00:00 UTC"
    /// );
    /// ```
    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err>;

    /// Return a copy truncated by TimeDelta.
    ///
    /// # Example
    /// ``` rust
    /// # use chrono::{DurationRound, TimeDelta, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 11)
    ///     .unwrap()
    ///     .and_hms_milli_opt(12, 0, 0, 154)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(
    ///     dt.duration_trunc(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
    ///     "2018-01-11 12:00:00.150 UTC"
    /// );
    /// assert_eq!(
    ///     dt.duration_trunc(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
    ///     "2018-01-11 00:00:00 UTC"
    /// );
    /// ```
    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err>;

    /// Return a copy rounded **up** by TimeDelta.
    ///
    /// # Example
    /// ``` rust
    /// # use chrono::{DurationRound, TimeDelta, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 11)
    ///     .unwrap()
    ///     .and_hms_milli_opt(12, 0, 0, 154)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(
    ///     dt.duration_round_up(TimeDelta::milliseconds(10)).unwrap().to_string(),
    ///     "2018-01-11 12:00:00.160 UTC"
    /// );
    /// assert_eq!(
    ///     dt.duration_round_up(TimeDelta::hours(1)).unwrap().to_string(),
    ///     "2018-01-11 13:00:00 UTC"
    /// );
    ///
    /// assert_eq!(
    ///     dt.duration_round_up(TimeDelta::days(1)).unwrap().to_string(),
    ///     "2018-01-12 00:00:00 UTC"
    /// );
    /// ```
    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err>;
}

impl<Tz: TimeZone> DurationRound for DateTime<Tz> {
    type Err = RoundingError;

    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round(self.naive_local(), self, duration)
    }

    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_trunc(self.naive_local(), self, duration)
    }

    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round_up(self.naive_local(), self, duration)
    }
}

impl DurationRound for NaiveDateTime {
    type Err = RoundingError;

    fn duration_round(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round(self, self, duration)
    }

    fn duration_trunc(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_trunc(self, self, duration)
    }

    fn duration_round_up(self, duration: TimeDelta) -> Result<Self, Self::Err> {
        duration_round_up(self, self, duration)
    }
}

fn duration_round<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp =
            naive.and_utc().timestamp_nanos_opt().ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        if delta_down == 0 {
            Ok(original)
        } else {
            let (delta_up, delta_down) = if delta_down < 0 {
                (delta_down.abs(), span - delta_down.abs())
            } else {
                (span - delta_down, delta_down)
            };
            if delta_up <= delta_down {
                Ok(original + TimeDelta::nanoseconds(delta_up))
            } else {
                Ok(original - TimeDelta::nanoseconds(delta_down))
            }
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}

fn duration_trunc<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp =
            naive.and_utc().timestamp_nanos_opt().ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        match delta_down.cmp(&0) {
            Ordering::Equal => Ok(original),
            Ordering::Greater => Ok(original - TimeDelta::nanoseconds(delta_down)),
            Ordering::Less => Ok(original - TimeDelta::nanoseconds(span - delta_down.abs())),
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}

fn duration_round_up<T>(
    naive: NaiveDateTime,
    original: T,
    duration: TimeDelta,
) -> Result<T, RoundingError>
where
    T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>,
{
    if let Some(span) = duration.num_nanoseconds() {
        if span <= 0 {
            return Err(RoundingError::DurationExceedsLimit);
        }
        let stamp =
            naive.and_utc().timestamp_nanos_opt().ok_or(RoundingError::TimestampExceedsLimit)?;
        let delta_down = stamp % span;
        match delta_down.cmp(&0) {
            Ordering::Equal => Ok(original),
            Ordering::Greater => Ok(original + TimeDelta::nanoseconds(span - delta_down)),
            Ordering::Less => Ok(original + TimeDelta::nanoseconds(delta_down.abs())),
        }
    } else {
        Err(RoundingError::DurationExceedsLimit)
    }
}

/// An error from rounding by `TimeDelta`
///
/// See: [`DurationRound`]
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum RoundingError {
    /// Error when the TimeDelta exceeds the TimeDelta from or until the Unix epoch.
    ///
    /// Note: this error is not produced anymore.
    DurationExceedsTimestamp,

    /// Error when `TimeDelta.num_nanoseconds` exceeds the limit.
    ///
    /// ``` rust
    /// # use chrono::{DurationRound, TimeDelta, RoundingError, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2260, 12, 31)
    ///     .unwrap()
    ///     .and_hms_nano_opt(23, 59, 59, 1_75_500_000)
    ///     .unwrap()
    ///     .and_utc();
    ///
    /// assert_eq!(
    ///     dt.duration_round(TimeDelta::try_days(300 * 365).unwrap()),
    ///     Err(RoundingError::DurationExceedsLimit)
    /// );
    /// ```
    DurationExceedsLimit,

    /// Error when `DateTime.timestamp_nanos` exceeds the limit.
    ///
    /// ``` rust
    /// # use chrono::{DurationRound, TimeDelta, RoundingError, TimeZone, Utc};
    /// let dt = Utc.with_ymd_and_hms(2300, 12, 12, 0, 0, 0).unwrap();
    ///
    /// assert_eq!(
    ///     dt.duration_round(TimeDelta::try_days(1).unwrap()),
    ///     Err(RoundingError::TimestampExceedsLimit)
    /// );
    /// ```
    TimestampExceedsLimit,
}

impl fmt::Display for RoundingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RoundingError::DurationExceedsTimestamp => {
                write!(f, "duration in nanoseconds exceeds timestamp")
            }
            RoundingError::DurationExceedsLimit => {
                write!(f, "duration exceeds num_nanoseconds limit")
            }
            RoundingError::TimestampExceedsLimit => {
                write!(f, "timestamp exceeds num_nanoseconds limit")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RoundingError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "error from rounding or truncating with DurationRound"
    }
}

#[cfg(all(not(feature = "std"), feature = "core-error"))]
impl core::error::Error for RoundingError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "error from rounding or truncating with DurationRound"
    }
}

#[cfg(test)]
mod tests {
    use super::{DurationRound, RoundingError, SubsecRound, TimeDelta};
    use crate::Timelike;
    use crate::offset::{FixedOffset, TimeZone, Utc};
    use crate::{DateTime, NaiveDate};

    #[test]
    fn test_round_subsecs() {
        let pst = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let dt = pst
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2018, 1, 11)
                    .unwrap()
                    .and_hms_nano_opt(10, 5, 13, 84_660_684)
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(dt.round_subsecs(10), dt);
        assert_eq!(dt.round_subsecs(9), dt);
        assert_eq!(dt.round_subsecs(8).nanosecond(), 84_660_680);
        assert_eq!(dt.round_subsecs(7).nanosecond(), 84_660_700);
        assert_eq!(dt.round_subsecs(6).nanosecond(), 84_661_000);
        assert_eq!(dt.round_subsecs(5).nanosecond(), 84_660_000);
        assert_eq!(dt.round_subsecs(4).nanosecond(), 84_700_000);
        assert_eq!(dt.round_subsecs(3).nanosecond(), 85_000_000);
        assert_eq!(dt.round_subsecs(2).nanosecond(), 80_000_000);
        assert_eq!(dt.round_subsecs(1).nanosecond(), 100_000_000);

        assert_eq!(dt.round_subsecs(0).nanosecond(), 0);
        assert_eq!(dt.round_subsecs(0).second(), 13);

        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2018, 1, 11)
                    .unwrap()
                    .and_hms_nano_opt(10, 5, 27, 750_500_000)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(dt.round_subsecs(9), dt);
        assert_eq!(dt.round_subsecs(4), dt);
        assert_eq!(dt.round_subsecs(3).nanosecond(), 751_000_000);
        assert_eq!(dt.round_subsecs(2).nanosecond(), 750_000_000);
        assert_eq!(dt.round_subsecs(1).nanosecond(), 800_000_000);

        assert_eq!(dt.round_subsecs(0).nanosecond(), 0);
        assert_eq!(dt.round_subsecs(0).second(), 28);
    }

    #[test]
    fn test_round_leap_nanos() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 1_750_500_000)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(dt.round_subsecs(9), dt);
        assert_eq!(dt.round_subsecs(4), dt);
        assert_eq!(dt.round_subsecs(2).nanosecond(), 1_750_000_000);
        assert_eq!(dt.round_subsecs(1).nanosecond(), 1_800_000_000);
        assert_eq!(dt.round_subsecs(1).second(), 59);

        assert_eq!(dt.round_subsecs(0).nanosecond(), 0);
        assert_eq!(dt.round_subsecs(0).second(), 0);
    }

    #[test]
    fn test_trunc_subsecs() {
        let pst = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let dt = pst
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2018, 1, 11)
                    .unwrap()
                    .and_hms_nano_opt(10, 5, 13, 84_660_684)
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(dt.trunc_subsecs(10), dt);
        assert_eq!(dt.trunc_subsecs(9), dt);
        assert_eq!(dt.trunc_subsecs(8).nanosecond(), 84_660_680);
        assert_eq!(dt.trunc_subsecs(7).nanosecond(), 84_660_600);
        assert_eq!(dt.trunc_subsecs(6).nanosecond(), 84_660_000);
        assert_eq!(dt.trunc_subsecs(5).nanosecond(), 84_660_000);
        assert_eq!(dt.trunc_subsecs(4).nanosecond(), 84_600_000);
        assert_eq!(dt.trunc_subsecs(3).nanosecond(), 84_000_000);
        assert_eq!(dt.trunc_subsecs(2).nanosecond(), 80_000_000);
        assert_eq!(dt.trunc_subsecs(1).nanosecond(), 0);

        assert_eq!(dt.trunc_subsecs(0).nanosecond(), 0);
        assert_eq!(dt.trunc_subsecs(0).second(), 13);

        let dt = pst
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2018, 1, 11)
                    .unwrap()
                    .and_hms_nano_opt(10, 5, 27, 750_500_000)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(dt.trunc_subsecs(9), dt);
        assert_eq!(dt.trunc_subsecs(4), dt);
        assert_eq!(dt.trunc_subsecs(3).nanosecond(), 750_000_000);
        assert_eq!(dt.trunc_subsecs(2).nanosecond(), 750_000_000);
        assert_eq!(dt.trunc_subsecs(1).nanosecond(), 700_000_000);

        assert_eq!(dt.trunc_subsecs(0).nanosecond(), 0);
        assert_eq!(dt.trunc_subsecs(0).second(), 27);
    }

    #[test]
    fn test_trunc_leap_nanos() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 1_750_500_000)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(dt.trunc_subsecs(9), dt);
        assert_eq!(dt.trunc_subsecs(4), dt);
        assert_eq!(dt.trunc_subsecs(2).nanosecond(), 1_750_000_000);
        assert_eq!(dt.trunc_subsecs(1).nanosecond(), 1_700_000_000);
        assert_eq!(dt.trunc_subsecs(1).second(), 59);

        assert_eq!(dt.trunc_subsecs(0).nanosecond(), 1_000_000_000);
        assert_eq!(dt.trunc_subsecs(0).second(), 59);
    }

    #[test]
    fn test_duration_round() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 175_500_000)
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(
            dt.duration_round(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );
        assert_eq!(dt.duration_round(TimeDelta::zero()), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_round(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.180 UTC"
        );

        // round up
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 30, 0)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:25:00 UTC"
        );
        // round down
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 29, 999)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00 UTC"
        );

        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00 UTC"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00 UTC"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00 UTC"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-13 00:00:00 UTC"
        );

        // timezone east
        let dt =
            FixedOffset::east_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_round(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-28 00:00:00 +01:00"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-29 00:00:00 +01:00"
        );

        // timezone west
        let dt =
            FixedOffset::west_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_round(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-28 00:00:00 -01:00"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-29 00:00:00 -01:00"
        );
    }

    #[test]
    fn test_duration_round_naive() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 175_500_000)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();

        assert_eq!(
            dt.duration_round(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );
        assert_eq!(dt.duration_round(TimeDelta::zero()), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_round(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.180"
        );

        // round up
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 30, 0)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:25:00"
        );
        // round down
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 29, 999)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00"
        );

        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00"
        );
        assert_eq!(
            dt.duration_round(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-13 00:00:00"
        );
    }

    #[test]
    fn test_duration_round_pre_epoch() {
        let dt = Utc.with_ymd_and_hms(1969, 12, 12, 12, 12, 12).unwrap();
        assert_eq!(
            dt.duration_round(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "1969-12-12 12:10:00 UTC"
        );
    }

    #[test]
    fn test_duration_trunc() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 175_500_000)
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(
            dt.duration_trunc(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );
        assert_eq!(dt.duration_trunc(TimeDelta::zero()), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_trunc(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.170 UTC"
        );

        // would round up
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 30, 0)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00 UTC"
        );
        // would round down
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 29, 999)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00 UTC"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00 UTC"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00 UTC"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00 UTC"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-12 00:00:00 UTC"
        );

        // timezone east
        let dt =
            FixedOffset::east_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-27 00:00:00 +01:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-22 00:00:00 +01:00"
        );

        // timezone west
        let dt =
            FixedOffset::west_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-27 00:00:00 -01:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-22 00:00:00 -01:00"
        );
    }

    #[test]
    fn test_duration_trunc_naive() {
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2016, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 175_500_000)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();

        assert_eq!(
            dt.duration_trunc(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );
        assert_eq!(dt.duration_trunc(TimeDelta::zero()), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_trunc(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.170"
        );

        // would round up
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 30, 0)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00"
        );
        // would round down
        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 29, 999)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:20:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 18:00:00"
        );
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-12 00:00:00"
        );
    }

    #[test]
    fn test_duration_trunc_pre_epoch() {
        let dt = Utc.with_ymd_and_hms(1969, 12, 12, 12, 12, 12).unwrap();
        assert_eq!(
            dt.duration_trunc(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "1969-12-12 12:10:00 UTC"
        );
    }

    #[test]
    fn issue1010() {
        let dt = DateTime::from_timestamp(-4_227_854_320, 678_774_288).unwrap();
        let span = TimeDelta::microseconds(-7_019_067_213_869_040);
        assert_eq!(dt.duration_trunc(span), Err(RoundingError::DurationExceedsLimit));

        let dt = DateTime::from_timestamp(320_041_586, 920_103_021).unwrap();
        let span = TimeDelta::nanoseconds(-8_923_838_508_697_114_584);
        assert_eq!(dt.duration_round(span), Err(RoundingError::DurationExceedsLimit));

        let dt = DateTime::from_timestamp(-2_621_440, 0).unwrap();
        let span = TimeDelta::nanoseconds(-9_223_372_036_854_771_421);
        assert_eq!(dt.duration_round(span), Err(RoundingError::DurationExceedsLimit));
    }

    #[test]
    fn test_duration_trunc_close_to_epoch() {
        let span = TimeDelta::try_minutes(15).unwrap();

        let dt = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().and_hms_opt(0, 0, 15).unwrap();
        assert_eq!(dt.duration_trunc(span).unwrap().to_string(), "1970-01-01 00:00:00");

        let dt = NaiveDate::from_ymd_opt(1969, 12, 31).unwrap().and_hms_opt(23, 59, 45).unwrap();
        assert_eq!(dt.duration_trunc(span).unwrap().to_string(), "1969-12-31 23:45:00");
    }

    #[test]
    fn test_duration_round_close_to_epoch() {
        let span = TimeDelta::try_minutes(15).unwrap();

        let dt = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().and_hms_opt(0, 0, 15).unwrap();
        assert_eq!(dt.duration_round(span).unwrap().to_string(), "1970-01-01 00:00:00");

        let dt = NaiveDate::from_ymd_opt(1969, 12, 31).unwrap().and_hms_opt(23, 59, 45).unwrap();
        assert_eq!(dt.duration_round(span).unwrap().to_string(), "1970-01-01 00:00:00");
    }

    #[test]
    fn test_duration_round_close_to_min_max() {
        let span = TimeDelta::nanoseconds(i64::MAX);

        let dt = DateTime::from_timestamp_nanos(i64::MIN / 2 - 1);
        assert_eq!(
            dt.duration_round(span).unwrap().to_string(),
            "1677-09-21 00:12:43.145224193 UTC"
        );

        let dt = DateTime::from_timestamp_nanos(i64::MIN / 2 + 1);
        assert_eq!(dt.duration_round(span).unwrap().to_string(), "1970-01-01 00:00:00 UTC");

        let dt = DateTime::from_timestamp_nanos(i64::MAX / 2 + 1);
        assert_eq!(
            dt.duration_round(span).unwrap().to_string(),
            "2262-04-11 23:47:16.854775807 UTC"
        );

        let dt = DateTime::from_timestamp_nanos(i64::MAX / 2 - 1);
        assert_eq!(dt.duration_round(span).unwrap().to_string(), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn test_duration_round_up() {
        let dt = NaiveDate::from_ymd_opt(2016, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 175_500_000)
            .unwrap()
            .and_utc();

        assert_eq!(
            dt.duration_round_up(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );

        assert_eq!(
            dt.duration_round_up(TimeDelta::zero()),
            Err(RoundingError::DurationExceedsLimit)
        );

        assert_eq!(dt.duration_round_up(TimeDelta::MAX), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_round_up(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.180 UTC"
        );

        // round up
        let dt = NaiveDate::from_ymd_opt(2012, 12, 12)
            .unwrap()
            .and_hms_milli_opt(18, 22, 30, 0)
            .unwrap()
            .and_utc();

        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:25:00 UTC"
        );

        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00 UTC"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00 UTC"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 19:00:00 UTC"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-13 00:00:00 UTC"
        );

        // timezone east
        let dt =
            FixedOffset::east_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-28 00:00:00 +01:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-29 00:00:00 +01:00"
        );

        // timezone west
        let dt =
            FixedOffset::west_opt(3600).unwrap().with_ymd_and_hms(2020, 10, 27, 15, 0, 0).unwrap();
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2020-10-28 00:00:00 -01:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_weeks(1).unwrap()).unwrap().to_string(),
            "2020-10-29 00:00:00 -01:00"
        );
    }

    #[test]
    fn test_duration_round_up_naive() {
        let dt = NaiveDate::from_ymd_opt(2016, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 175_500_000)
            .unwrap();

        assert_eq!(
            dt.duration_round_up(TimeDelta::new(-1, 0).unwrap()),
            Err(RoundingError::DurationExceedsLimit)
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::zero()),
            Err(RoundingError::DurationExceedsLimit)
        );

        assert_eq!(dt.duration_round_up(TimeDelta::MAX), Err(RoundingError::DurationExceedsLimit));

        assert_eq!(
            dt.duration_round_up(TimeDelta::try_milliseconds(10).unwrap()).unwrap().to_string(),
            "2016-12-31 23:59:59.180"
        );

        let dt = Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2012, 12, 12)
                    .unwrap()
                    .and_hms_milli_opt(18, 22, 30, 0)
                    .unwrap(),
            )
            .unwrap()
            .naive_utc();
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(5).unwrap()).unwrap().to_string(),
            "2012-12-12 18:25:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(30).unwrap()).unwrap().to_string(),
            "2012-12-12 18:30:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_hours(1).unwrap()).unwrap().to_string(),
            "2012-12-12 19:00:00"
        );
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_days(1).unwrap()).unwrap().to_string(),
            "2012-12-13 00:00:00"
        );
    }

    #[test]
    fn test_duration_round_up_pre_epoch() {
        let dt = Utc.with_ymd_and_hms(1969, 12, 12, 12, 12, 12).unwrap();
        assert_eq!(
            dt.duration_round_up(TimeDelta::try_minutes(10).unwrap()).unwrap().to_string(),
            "1969-12-12 12:20:00 UTC"
        );

        let time_delta = TimeDelta::minutes(30);
        assert_eq!(
            DateTime::UNIX_EPOCH.duration_round_up(time_delta).unwrap().to_string(),
            "1970-01-01 00:00:00 UTC"
        )
    }

    #[test]
    fn test_duration_round_up_close_to_min_max() {
        let mut dt = NaiveDate::from_ymd_opt(2012, 12, 12)
            .unwrap()
            .and_hms_milli_opt(18, 22, 30, 0)
            .unwrap()
            .and_utc();

        let span = TimeDelta::nanoseconds(i64::MAX);

        assert_eq!(
            dt.duration_round_up(span).unwrap().to_string(),
            DateTime::from_timestamp_nanos(i64::MAX).to_string()
        );

        dt = DateTime::UNIX_EPOCH + TimeDelta::nanoseconds(1);
        assert_eq!(dt.duration_round_up(span).unwrap(), DateTime::from_timestamp_nanos(i64::MAX));

        let dt = DateTime::from_timestamp_nanos(1);
        assert_eq!(
            dt.duration_round_up(span).unwrap().to_string(),
            "2262-04-11 23:47:16.854775807 UTC"
        );

        let dt = DateTime::from_timestamp_nanos(-1);
        assert_eq!(dt.duration_round_up(span).unwrap(), DateTime::UNIX_EPOCH);

        // Rounds to 1677-09-21 00:12:43.145224193 UTC if at i64::MIN.
        // because i64::MIN is 1677-09-21 00:12:43.145224192 UTC.
        //
        //                                                v
        // We add 2 to get to 1677-09-21 00:12:43.145224194 UTC
        // this issue is because abs(i64::MIN) == i64::MAX + 1
        let dt = DateTime::from_timestamp_nanos(i64::MIN + 2);
        assert_eq!(dt.duration_round_up(span).unwrap(), DateTime::UNIX_EPOCH);
    }
}
