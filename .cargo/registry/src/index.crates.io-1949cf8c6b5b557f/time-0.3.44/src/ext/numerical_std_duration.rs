use core::time::Duration as StdDuration;

use crate::convert::*;

/// Sealed trait to prevent downstream implementations.
mod sealed {
    /// A trait that cannot be implemented by downstream users.
    pub trait Sealed {}
    impl Sealed for u64 {}
    impl Sealed for f64 {}
}

/// Create [`std::time::Duration`]s from numeric literals.
///
/// # Examples
///
/// Basic construction of [`std::time::Duration`]s.
///
/// ```rust
/// # use time::ext::NumericalStdDuration;
/// # use core::time::Duration;
/// assert_eq!(5.std_nanoseconds(), Duration::from_nanos(5));
/// assert_eq!(5.std_microseconds(), Duration::from_micros(5));
/// assert_eq!(5.std_milliseconds(), Duration::from_millis(5));
/// assert_eq!(5.std_seconds(), Duration::from_secs(5));
/// assert_eq!(5.std_minutes(), Duration::from_secs(5 * 60));
/// assert_eq!(5.std_hours(), Duration::from_secs(5 * 3_600));
/// assert_eq!(5.std_days(), Duration::from_secs(5 * 86_400));
/// assert_eq!(5.std_weeks(), Duration::from_secs(5 * 604_800));
/// ```
///
/// Just like any other [`std::time::Duration`], they can be added, subtracted, etc.
///
/// ```rust
/// # use time::ext::NumericalStdDuration;
/// assert_eq!(
///     2.std_seconds() + 500.std_milliseconds(),
///     2_500.std_milliseconds()
/// );
/// assert_eq!(
///     2.std_seconds() - 500.std_milliseconds(),
///     1_500.std_milliseconds()
/// );
/// ```
///
/// When called on floating point values, any remainder of the floating point value will be
/// truncated. Keep in mind that floating point numbers are inherently imprecise and have
/// limited capacity.
#[diagnostic::on_unimplemented(note = "this extension trait is intended to be used with numeric \
                                       literals, such as `5.std_seconds()`")]
pub trait NumericalStdDuration: sealed::Sealed {
    /// Create a [`std::time::Duration`] from the number of nanoseconds.
    fn std_nanoseconds(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of microseconds.
    fn std_microseconds(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of milliseconds.
    fn std_milliseconds(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of seconds.
    fn std_seconds(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of minutes.
    fn std_minutes(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of hours.
    fn std_hours(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of days.
    fn std_days(self) -> StdDuration;
    /// Create a [`std::time::Duration`] from the number of weeks.
    fn std_weeks(self) -> StdDuration;
}

impl NumericalStdDuration for u64 {
    #[inline]
    fn std_nanoseconds(self) -> StdDuration {
        StdDuration::from_nanos(self)
    }

    #[inline]
    fn std_microseconds(self) -> StdDuration {
        StdDuration::from_micros(self)
    }

    #[inline]
    fn std_milliseconds(self) -> StdDuration {
        StdDuration::from_millis(self)
    }

    #[inline]
    fn std_seconds(self) -> StdDuration {
        StdDuration::from_secs(self)
    }

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn std_minutes(self) -> StdDuration {
        StdDuration::from_secs(
            self.checked_mul(Second::per_t(Minute))
                .expect("overflow constructing `time::Duration`"),
        )
    }

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn std_hours(self) -> StdDuration {
        StdDuration::from_secs(
            self.checked_mul(Second::per_t(Hour))
                .expect("overflow constructing `time::Duration`"),
        )
    }

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn std_days(self) -> StdDuration {
        StdDuration::from_secs(
            self.checked_mul(Second::per_t(Day))
                .expect("overflow constructing `time::Duration`"),
        )
    }

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn std_weeks(self) -> StdDuration {
        StdDuration::from_secs(
            self.checked_mul(Second::per_t(Week))
                .expect("overflow constructing `time::Duration`"),
        )
    }
}

impl NumericalStdDuration for f64 {
    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_nanoseconds(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos(self as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_microseconds(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Microsecond)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_milliseconds(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Millisecond)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_seconds(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Second)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_minutes(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Minute)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_hours(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Hour)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_days(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Day)) as u64)
    }

    /// # Panics
    ///
    /// This will panic if self is negative.
    #[inline]
    #[track_caller]
    fn std_weeks(self) -> StdDuration {
        assert!(self >= 0.);
        StdDuration::from_nanos((self * Nanosecond::per_t::<Self>(Week)) as u64)
    }
}
