use std::time::Instant as StdInstant;

use crate::Duration;

/// Sealed trait to prevent downstream implementations.
mod sealed {
    /// A trait that cannot be implemented by downstream users.
    pub trait Sealed: Sized {}
    impl Sealed for std::time::Instant {}
}

/// An extension trait for [`std::time::Instant`] that adds methods for
/// [`time::Duration`](Duration)s.
pub trait InstantExt: sealed::Sealed {
    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`InstantExt::checked_add_signed`] for a non-panicking
    /// version.
    #[inline]
    #[track_caller]
    fn add_signed(self, duration: Duration) -> Self {
        self.checked_add_signed(duration)
            .expect("overflow when adding duration to instant")
    }

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`InstantExt::checked_sub_signed`] for a non-panicking
    /// version.
    #[inline]
    #[track_caller]
    fn sub_signed(self, duration: Duration) -> Self {
        self.checked_sub_signed(duration)
            .expect("overflow when subtracting duration from instant")
    }

    /// Returns `Some(t)` where `t` is the time `self.checked_add_signed(duration)` if `t` can be
    /// represented as `Instant` (which means it's inside the bounds of the underlying data
    /// structure), `None` otherwise.
    fn checked_add_signed(&self, duration: Duration) -> Option<Self>;

    /// Returns `Some(t)` where `t` is the time `self.checked_sub_signed(duration)` if `t` can be
    /// represented as `Instant` (which means it's inside the bounds of the underlying data
    /// structure), `None` otherwise.
    fn checked_sub_signed(&self, duration: Duration) -> Option<Self>;

    /// Returns the amount of time elapsed from another instant to this one. This will be negative
    /// if `earlier` is later than `self`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::thread::sleep;
    /// # use std::time::{Duration, Instant};
    /// # use time::ext::InstantExt;
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.signed_duration_since(now)); // positive
    /// println!("{:?}", now.signed_duration_since(new_now)); // negative
    /// ```
    fn signed_duration_since(&self, earlier: Self) -> Duration;
}

impl InstantExt for StdInstant {
    #[inline]
    fn checked_add_signed(&self, duration: Duration) -> Option<Self> {
        if duration.is_positive() {
            self.checked_add(duration.unsigned_abs())
        } else if duration.is_negative() {
            self.checked_sub(duration.unsigned_abs())
        } else {
            debug_assert!(duration.is_zero());
            Some(*self)
        }
    }

    #[inline]
    fn checked_sub_signed(&self, duration: Duration) -> Option<Self> {
        if duration.is_positive() {
            self.checked_sub(duration.unsigned_abs())
        } else if duration.is_negative() {
            self.checked_add(duration.unsigned_abs())
        } else {
            debug_assert!(duration.is_zero());
            Some(*self)
        }
    }

    #[inline]
    fn signed_duration_since(&self, earlier: Self) -> Duration {
        if *self > earlier {
            self.saturating_duration_since(earlier)
                .try_into()
                .unwrap_or(Duration::MAX)
        } else {
            earlier
                .saturating_duration_since(*self)
                .try_into()
                .map_or(Duration::MIN, |d: Duration| -d)
        }
    }
}
