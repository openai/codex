use core::cmp::Ordering;
use core::ops::Sub;

use crate::{Duration, OffsetDateTime, UtcDateTime};

impl Sub<OffsetDateTime> for UtcDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: OffsetDateTime) -> Self::Output {
        OffsetDateTime::from(self) - rhs
    }
}

impl Sub<UtcDateTime> for OffsetDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: UtcDateTime) -> Self::Output {
        self - Self::from(rhs)
    }
}

impl PartialEq<OffsetDateTime> for UtcDateTime {
    #[inline]
    fn eq(&self, other: &OffsetDateTime) -> bool {
        OffsetDateTime::from(*self) == *other
    }
}

impl PartialEq<UtcDateTime> for OffsetDateTime {
    #[inline]
    fn eq(&self, other: &UtcDateTime) -> bool {
        *self == Self::from(*other)
    }
}

impl PartialOrd<OffsetDateTime> for UtcDateTime {
    #[inline]
    fn partial_cmp(&self, other: &OffsetDateTime) -> Option<Ordering> {
        OffsetDateTime::from(*self).partial_cmp(other)
    }
}

impl PartialOrd<UtcDateTime> for OffsetDateTime {
    #[inline]
    fn partial_cmp(&self, other: &UtcDateTime) -> Option<Ordering> {
        self.partial_cmp(&Self::from(*other))
    }
}

impl From<OffsetDateTime> for UtcDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn from(datetime: OffsetDateTime) -> Self {
        datetime.to_utc()
    }
}

impl From<UtcDateTime> for OffsetDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn from(datetime: UtcDateTime) -> Self {
        datetime.as_primitive().assume_utc()
    }
}
