use std::time::{Duration, Instant as MonotonicInstant};

/// A little helper for representing expiration time.
///
/// An overflowing expiration time is treated identically to a time that is
/// always expired.
///
/// When `None` internally, it implies that the expiration time is at some
/// arbitrary point in the past beyond all possible "time to live" values.
/// i.e., A `None` value invalidates the cache at the next failed lookup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Expiration(Option<MonotonicInstant>);

impl Expiration {
    /// Returns an expiration time for which `is_expired` returns true after
    /// the given duration has elapsed from this instant.
    pub(crate) fn after(ttl: Duration) -> Expiration {
        Expiration(
            crate::now::monotonic_time().and_then(|now| now.checked_add(ttl)),
        )
    }

    /// Returns an expiration time for which `is_expired` always returns true.
    pub(crate) const fn expired() -> Expiration {
        Expiration(None)
    }

    /// Whether expiration has occurred or not.
    pub(crate) fn is_expired(self) -> bool {
        self.0.map_or(true, |t| {
            let Some(now) = crate::now::monotonic_time() else { return true };
            now > t
        })
    }
}

impl core::fmt::Display for Expiration {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let Some(instant) = self.0 else {
            return write!(f, "expired");
        };
        let Some(now) = crate::now::monotonic_time() else {
            return write!(f, "expired");
        };
        let Some(duration) = instant.checked_duration_since(now) else {
            return write!(f, "expired");
        };
        write!(f, "{duration:?}")
    }
}
