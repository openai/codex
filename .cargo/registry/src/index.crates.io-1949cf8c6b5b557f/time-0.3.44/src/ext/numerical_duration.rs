use crate::convert::*;
use crate::Duration;

/// Sealed trait to prevent downstream implementations.
mod sealed {
    /// A trait that cannot be implemented by downstream users.
    pub trait Sealed {}
    impl Sealed for i64 {}
    impl Sealed for f64 {}
}

/// Create [`Duration`]s from numeric literals.
///
/// # Examples
///
/// Basic construction of [`Duration`]s.
///
/// ```rust
/// # use time::{Duration, ext::NumericalDuration};
/// assert_eq!(5.nanoseconds(), Duration::nanoseconds(5));
/// assert_eq!(5.microseconds(), Duration::microseconds(5));
/// assert_eq!(5.milliseconds(), Duration::milliseconds(5));
/// assert_eq!(5.seconds(), Duration::seconds(5));
/// assert_eq!(5.minutes(), Duration::minutes(5));
/// assert_eq!(5.hours(), Duration::hours(5));
/// assert_eq!(5.days(), Duration::days(5));
/// assert_eq!(5.weeks(), Duration::weeks(5));
/// ```
///
/// Signed integers work as well!
///
/// ```rust
/// # use time::{Duration, ext::NumericalDuration};
/// assert_eq!((-5).nanoseconds(), Duration::nanoseconds(-5));
/// assert_eq!((-5).microseconds(), Duration::microseconds(-5));
/// assert_eq!((-5).milliseconds(), Duration::milliseconds(-5));
/// assert_eq!((-5).seconds(), Duration::seconds(-5));
/// assert_eq!((-5).minutes(), Duration::minutes(-5));
/// assert_eq!((-5).hours(), Duration::hours(-5));
/// assert_eq!((-5).days(), Duration::days(-5));
/// assert_eq!((-5).weeks(), Duration::weeks(-5));
/// ```
///
/// Just like any other [`Duration`], they can be added, subtracted, etc.
///
/// ```rust
/// # use time::ext::NumericalDuration;
/// assert_eq!(2.seconds() + 500.milliseconds(), 2_500.milliseconds());
/// assert_eq!(2.seconds() - 500.milliseconds(), 1_500.milliseconds());
/// ```
///
/// When called on floating point values, any remainder of the floating point value will be
/// truncated. Keep in mind that floating point numbers are inherently imprecise and have
/// limited capacity.
#[diagnostic::on_unimplemented(note = "this extension trait is intended to be used with numeric \
                                       literals, such as `5.seconds()`")]
pub trait NumericalDuration: sealed::Sealed {
    /// Create a [`Duration`] from the number of nanoseconds.
    fn nanoseconds(self) -> Duration;
    /// Create a [`Duration`] from the number of microseconds.
    fn microseconds(self) -> Duration;
    /// Create a [`Duration`] from the number of milliseconds.
    fn milliseconds(self) -> Duration;
    /// Create a [`Duration`] from the number of seconds.
    fn seconds(self) -> Duration;
    /// Create a [`Duration`] from the number of minutes.
    fn minutes(self) -> Duration;
    /// Create a [`Duration`] from the number of hours.
    fn hours(self) -> Duration;
    /// Create a [`Duration`] from the number of days.
    fn days(self) -> Duration;
    /// Create a [`Duration`] from the number of weeks.
    fn weeks(self) -> Duration;
}

impl NumericalDuration for i64 {
    #[inline]
    fn nanoseconds(self) -> Duration {
        Duration::nanoseconds(self)
    }

    #[inline]
    fn microseconds(self) -> Duration {
        Duration::microseconds(self)
    }

    #[inline]
    fn milliseconds(self) -> Duration {
        Duration::milliseconds(self)
    }

    #[inline]
    fn seconds(self) -> Duration {
        Duration::seconds(self)
    }

    #[inline]
    #[track_caller]
    fn minutes(self) -> Duration {
        Duration::minutes(self)
    }

    #[inline]
    #[track_caller]
    fn hours(self) -> Duration {
        Duration::hours(self)
    }

    #[inline]
    #[track_caller]
    fn days(self) -> Duration {
        Duration::days(self)
    }

    #[inline]
    #[track_caller]
    fn weeks(self) -> Duration {
        Duration::weeks(self)
    }
}

impl NumericalDuration for f64 {
    #[inline]
    fn nanoseconds(self) -> Duration {
        Duration::nanoseconds(self as i64)
    }

    #[inline]
    fn microseconds(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Microsecond)) as i64)
    }

    #[inline]
    fn milliseconds(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Millisecond)) as i64)
    }

    #[inline]
    fn seconds(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Second)) as i64)
    }

    #[inline]
    #[track_caller]
    fn minutes(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Minute)) as i64)
    }

    #[inline]
    #[track_caller]
    fn hours(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Hour)) as i64)
    }

    #[inline]
    #[track_caller]
    fn days(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Day)) as i64)
    }

    #[inline]
    #[track_caller]
    fn weeks(self) -> Duration {
        Duration::nanoseconds((self * Nanosecond::per_t::<Self>(Week)) as i64)
    }
}
