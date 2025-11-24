//! The [`Instant`] struct and its associated `impl`s.

#![expect(deprecated)]

use core::borrow::Borrow;
use core::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use core::ops::{Add, Sub};
use core::time::Duration as StdDuration;
use std::time::Instant as StdInstant;

use crate::internal_macros::{impl_add_assign, impl_sub_assign};
use crate::Duration;

/// A measurement of a monotonically non-decreasing clock. Opaque and useful only with [`Duration`].
///
/// Instants are always guaranteed to be no less than any previously measured instant when created,
/// and are often useful for tasks such as measuring benchmarks or timing how long an operation
/// takes.
///
/// Note, however, that instants are not guaranteed to be **steady**. In other words, each tick of
/// the underlying clock may not be the same length (e.g. some seconds may be longer than others).
/// An instant may jump forwards or experience time dilation (slow down or speed up), but it will
/// never go backwards.
///
/// Instants are opaque types that can only be compared to one another. There is no method to get
/// "the number of seconds" from an instant. Instead, it only allows measuring the duration between
/// two instants (or comparing two instants).
///
/// This implementation allows for operations with signed [`Duration`]s, but is otherwise identical
/// to [`std::time::Instant`].
#[doc(hidden)]
#[deprecated(
    since = "0.3.35",
    note = "import `std::time::Instant` and `time::ext::InstantExt` instead"
)]
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(pub StdInstant);

impl Instant {
    /// Returns an `Instant` corresponding to "now".
    ///
    /// ```rust
    /// # #![expect(deprecated)]
    /// # use time::Instant;
    /// println!("{:?}", Instant::now());
    /// ```
    #[inline]
    pub fn now() -> Self {
        Self(StdInstant::now())
    }

    /// Returns the amount of time elapsed since this instant was created. The duration will always
    /// be nonnegative if the instant is not synthetically created.
    ///
    /// ```rust
    /// # #![expect(deprecated)]
    /// # use time::{Instant, ext::{NumericalStdDuration, NumericalDuration}};
    /// # use std::thread;
    /// let instant = Instant::now();
    /// thread::sleep(1.std_milliseconds());
    /// assert!(instant.elapsed() >= 1.milliseconds());
    /// ```
    #[inline]
    pub fn elapsed(self) -> Duration {
        Self::now() - self
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    ///
    /// ```rust
    /// # #![expect(deprecated)]
    /// # use time::{Instant, ext::NumericalDuration};
    /// let now = Instant::now();
    /// assert_eq!(now.checked_add(5.seconds()), Some(now + 5.seconds()));
    /// assert_eq!(now.checked_add((-5).seconds()), Some(now + (-5).seconds()));
    /// ```
    #[inline]
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        if duration.is_zero() {
            Some(self)
        } else if duration.is_positive() {
            self.0.checked_add(duration.unsigned_abs()).map(Self)
        } else {
            debug_assert!(duration.is_negative());
            self.0.checked_sub(duration.unsigned_abs()).map(Self)
        }
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    ///
    /// ```rust
    /// # #![expect(deprecated)]
    /// # use time::{Instant, ext::NumericalDuration};
    /// let now = Instant::now();
    /// assert_eq!(now.checked_sub(5.seconds()), Some(now - 5.seconds()));
    /// assert_eq!(now.checked_sub((-5).seconds()), Some(now - (-5).seconds()));
    /// ```
    #[inline]
    pub fn checked_sub(self, duration: Duration) -> Option<Self> {
        if duration.is_zero() {
            Some(self)
        } else if duration.is_positive() {
            self.0.checked_sub(duration.unsigned_abs()).map(Self)
        } else {
            debug_assert!(duration.is_negative());
            self.0.checked_add(duration.unsigned_abs()).map(Self)
        }
    }

    /// Obtain the inner [`std::time::Instant`].
    ///
    /// ```rust
    /// # #![expect(deprecated)]
    /// # use time::Instant;
    /// let now = Instant::now();
    /// assert_eq!(now.into_inner(), now.0);
    /// ```
    #[inline]
    pub const fn into_inner(self) -> StdInstant {
        self.0
    }
}

impl From<StdInstant> for Instant {
    #[inline]
    fn from(instant: StdInstant) -> Self {
        Self(instant)
    }
}

impl From<Instant> for StdInstant {
    #[inline]
    fn from(instant: Instant) -> Self {
        instant.0
    }
}

impl Sub for Instant {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    fn sub(self, other: Self) -> Self::Output {
        match self.0.cmp(&other.0) {
            Ordering::Equal => Duration::ZERO,
            Ordering::Greater => (self.0 - other.0)
                .try_into()
                .expect("overflow converting `std::time::Duration` to `time::Duration`"),
            Ordering::Less => -Duration::try_from(other.0 - self.0)
                .expect("overflow converting `std::time::Duration` to `time::Duration`"),
        }
    }
}

impl Sub<StdInstant> for Instant {
    type Output = Duration;

    #[inline]
    fn sub(self, other: StdInstant) -> Self::Output {
        self - Self(other)
    }
}

impl Sub<Instant> for StdInstant {
    type Output = Duration;

    #[inline]
    fn sub(self, other: Instant) -> Self::Output {
        Instant(self) - other
    }
}

impl Add<Duration> for Instant {
    type Output = Self;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure.
    #[inline]
    fn add(self, duration: Duration) -> Self::Output {
        if duration.is_positive() {
            Self(self.0 + duration.unsigned_abs())
        } else if duration.is_negative() {
            #[expect(clippy::unchecked_duration_subtraction)]
            Self(self.0 - duration.unsigned_abs())
        } else {
            debug_assert!(duration.is_zero());
            self
        }
    }
}

impl Add<Duration> for StdInstant {
    type Output = Self;

    #[inline]
    fn add(self, duration: Duration) -> Self::Output {
        (Instant(self) + duration).0
    }
}

impl Add<StdDuration> for Instant {
    type Output = Self;

    #[inline]
    fn add(self, duration: StdDuration) -> Self::Output {
        Self(self.0 + duration)
    }
}

impl_add_assign!(Instant: Duration, StdDuration);
impl_add_assign!(StdInstant: Duration);

impl Sub<Duration> for Instant {
    type Output = Self;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure.
    #[inline]
    fn sub(self, duration: Duration) -> Self::Output {
        if duration.is_positive() {
            #[expect(clippy::unchecked_duration_subtraction)]
            Self(self.0 - duration.unsigned_abs())
        } else if duration.is_negative() {
            Self(self.0 + duration.unsigned_abs())
        } else {
            debug_assert!(duration.is_zero());
            self
        }
    }
}

impl Sub<Duration> for StdInstant {
    type Output = Self;

    #[inline]
    fn sub(self, duration: Duration) -> Self::Output {
        (Instant(self) - duration).0
    }
}

impl Sub<StdDuration> for Instant {
    type Output = Self;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure.
    #[inline]
    fn sub(self, duration: StdDuration) -> Self::Output {
        #[expect(clippy::unchecked_duration_subtraction)]
        Self(self.0 - duration)
    }
}

impl_sub_assign!(Instant: Duration, StdDuration);
impl_sub_assign!(StdInstant: Duration);

impl PartialEq<StdInstant> for Instant {
    #[inline]
    fn eq(&self, rhs: &StdInstant) -> bool {
        self.0.eq(rhs)
    }
}

impl PartialEq<Instant> for StdInstant {
    #[inline]
    fn eq(&self, rhs: &Instant) -> bool {
        self.eq(&rhs.0)
    }
}

impl PartialOrd<StdInstant> for Instant {
    #[inline]
    fn partial_cmp(&self, rhs: &StdInstant) -> Option<Ordering> {
        self.0.partial_cmp(rhs)
    }
}

impl PartialOrd<Instant> for StdInstant {
    #[inline]
    fn partial_cmp(&self, rhs: &Instant) -> Option<Ordering> {
        self.partial_cmp(&rhs.0)
    }
}

impl AsRef<StdInstant> for Instant {
    #[inline]
    fn as_ref(&self) -> &StdInstant {
        &self.0
    }
}

impl Borrow<StdInstant> for Instant {
    #[inline]
    fn borrow(&self) -> &StdInstant {
        &self.0
    }
}
