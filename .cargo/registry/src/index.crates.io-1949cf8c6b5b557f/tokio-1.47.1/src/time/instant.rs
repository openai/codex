#![allow(clippy::trivially_copy_pass_by_ref)]

use std::fmt;
use std::ops;
use std::time::Duration;

/// A measurement of a monotonically nondecreasing clock.
/// Opaque and useful only with `Duration`.
///
/// Instants are always guaranteed to be no less than any previously measured
/// instant when created, and are often useful for tasks such as measuring
/// benchmarks or timing how long an operation takes.
///
/// Note, however, that instants are not guaranteed to be **steady**. In other
/// words, each tick of the underlying clock may not be the same length (e.g.
/// some seconds may be longer than others). An instant may jump forwards or
/// experience time dilation (slow down or speed up), but it will never go
/// backwards.
///
/// Instants are opaque types that can only be compared to one another. There is
/// no method to get "the number of seconds" from an instant. Instead, it only
/// allows measuring the duration between two instants (or comparing two
/// instants).
///
/// The size of an `Instant` struct may vary depending on the target operating
/// system.
///
/// # Note
///
/// This type wraps the inner `std` variant and is used to align the Tokio
/// clock for uses of `now()`. This can be useful for testing where you can
/// take advantage of `time::pause()` and `time::advance()`.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Instant {
    std: std::time::Instant,
}

impl Instant {
    /// Returns an instant corresponding to "now".
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time::Instant;
    ///
    /// let now = Instant::now();
    /// ```
    pub fn now() -> Instant {
        variant::now()
    }

    /// Create a `tokio::time::Instant` from a `std::time::Instant`.
    pub fn from_std(std: std::time::Instant) -> Instant {
        Instant { std }
    }

    pub(crate) fn far_future() -> Instant {
        // Roughly 30 years from now.
        // API does not provide a way to obtain max `Instant`
        // or convert specific date in the future to instant.
        // 1000 years overflows on macOS, 100 years overflows on FreeBSD.
        Self::now() + Duration::from_secs(86400 * 365 * 30)
    }

    /// Convert the value into a `std::time::Instant`.
    pub fn into_std(self) -> std::time::Instant {
        self.std
    }

    /// Returns the amount of time elapsed from another instant to this one, or
    /// zero duration if that instant is later than this one.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.std.saturating_duration_since(earlier.std)
    }

    /// Returns the amount of time elapsed from another instant to this one, or
    /// None if that instant is later than this one.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time::{Duration, Instant, sleep};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let now = Instant::now();
    ///     sleep(Duration::new(1, 0)).await;
    ///     let new_now = Instant::now();
    ///     println!("{:?}", new_now.checked_duration_since(now));
    ///     println!("{:?}", now.checked_duration_since(new_now)); // None
    /// }
    /// ```
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.std.checked_duration_since(earlier.std)
    }

    /// Returns the amount of time elapsed from another instant to this one, or
    /// zero duration if that instant is later than this one.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time::{Duration, Instant, sleep};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let now = Instant::now();
    ///     sleep(Duration::new(1, 0)).await;
    ///     let new_now = Instant::now();
    ///     println!("{:?}", new_now.saturating_duration_since(now));
    ///     println!("{:?}", now.saturating_duration_since(new_now)); // 0ns
    /// }
    /// ```
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.std.saturating_duration_since(earlier.std)
    }

    /// Returns the amount of time elapsed since this instant was created,
    /// or zero duration if this instant is in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::time::{Duration, Instant, sleep};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let instant = Instant::now();
    ///     let three_secs = Duration::from_secs(3);
    ///     sleep(three_secs).await;
    ///     assert!(instant.elapsed() >= three_secs);
    /// }
    /// ```
    pub fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(*self)
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be
    /// represented as `Instant` (which means it's inside the bounds of the
    /// underlying data structure), `None` otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.std.checked_add(duration).map(Instant::from_std)
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be
    /// represented as `Instant` (which means it's inside the bounds of the
    /// underlying data structure), `None` otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.std.checked_sub(duration).map(Instant::from_std)
    }
}

impl From<std::time::Instant> for Instant {
    fn from(time: std::time::Instant) -> Instant {
        Instant::from_std(time)
    }
}

impl From<Instant> for std::time::Instant {
    fn from(time: Instant) -> std::time::Instant {
        time.into_std()
    }
}

impl ops::Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, other: Duration) -> Instant {
        Instant::from_std(self.std + other)
    }
}

impl ops::AddAssign<Duration> for Instant {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Duration {
        self.std.saturating_duration_since(rhs.std)
    }
}

impl ops::Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Instant {
        Instant::from_std(std::time::Instant::sub(self.std, rhs))
    }
}

impl ops::SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.std.fmt(fmt)
    }
}

#[cfg(not(feature = "test-util"))]
mod variant {
    use super::Instant;

    pub(super) fn now() -> Instant {
        Instant::from_std(std::time::Instant::now())
    }
}

#[cfg(feature = "test-util")]
mod variant {
    use super::Instant;

    pub(super) fn now() -> Instant {
        crate::time::clock::now()
    }
}
