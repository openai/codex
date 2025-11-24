use core::cmp::Ordering;
use core::ops::Sub;
use std::time::SystemTime;

use crate::{Duration, UtcDateTime};

impl Sub<SystemTime> for UtcDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: SystemTime) -> Self::Output {
        self - Self::from(rhs)
    }
}

impl Sub<UtcDateTime> for SystemTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: UtcDateTime) -> Self::Output {
        UtcDateTime::from(self) - rhs
    }
}

impl PartialEq<SystemTime> for UtcDateTime {
    #[inline]
    fn eq(&self, rhs: &SystemTime) -> bool {
        self == &Self::from(*rhs)
    }
}

impl PartialEq<UtcDateTime> for SystemTime {
    #[inline]
    fn eq(&self, rhs: &UtcDateTime) -> bool {
        &UtcDateTime::from(*self) == rhs
    }
}

impl PartialOrd<SystemTime> for UtcDateTime {
    #[inline]
    fn partial_cmp(&self, other: &SystemTime) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl PartialOrd<UtcDateTime> for SystemTime {
    #[inline]
    fn partial_cmp(&self, other: &UtcDateTime) -> Option<Ordering> {
        UtcDateTime::from(*self).partial_cmp(other)
    }
}

impl From<SystemTime> for UtcDateTime {
    #[inline]
    fn from(system_time: SystemTime) -> Self {
        match system_time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => Self::UNIX_EPOCH + duration,
            Err(err) => Self::UNIX_EPOCH - err.duration(),
        }
    }
}

impl From<UtcDateTime> for SystemTime {
    #[inline]
    fn from(datetime: UtcDateTime) -> Self {
        let duration = datetime - UtcDateTime::UNIX_EPOCH;

        if duration.is_zero() {
            Self::UNIX_EPOCH
        } else if duration.is_positive() {
            Self::UNIX_EPOCH + duration.unsigned_abs()
        } else {
            debug_assert!(duration.is_negative());
            Self::UNIX_EPOCH - duration.unsigned_abs()
        }
    }
}
