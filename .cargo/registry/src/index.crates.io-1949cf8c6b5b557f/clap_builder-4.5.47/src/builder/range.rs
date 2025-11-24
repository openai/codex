/// Values per occurrence for an argument
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ValueRange {
    start_inclusive: usize,
    end_inclusive: usize,
}

impl ValueRange {
    /// Nor argument values, or a flag
    pub const EMPTY: Self = Self {
        start_inclusive: 0,
        end_inclusive: 0,
    };

    /// A single argument value, the most common case for options
    pub const SINGLE: Self = Self {
        start_inclusive: 1,
        end_inclusive: 1,
    };

    #[cfg(debug_assertions)]
    pub(crate) const OPTIONAL: Self = Self {
        start_inclusive: 0,
        end_inclusive: 1,
    };

    pub(crate) const FULL: Self = Self {
        start_inclusive: 0,
        end_inclusive: usize::MAX,
    };

    /// Create a range
    ///
    /// # Panics
    ///
    /// If the end is less than the start (debug builds)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::ValueRange;
    /// let range = ValueRange::new(5);
    /// let range = ValueRange::new(5..10);
    /// let range = ValueRange::new(5..=10);
    /// let range = ValueRange::new(5..);
    /// let range = ValueRange::new(..10);
    /// let range = ValueRange::new(..=10);
    /// ```
    ///
    /// While this will panic:
    /// ```should_panic
    /// # use clap_builder as clap;
    /// # use clap::builder::ValueRange;
    /// let range = ValueRange::new(10..5);  // Panics!
    /// ```
    pub fn new(range: impl Into<Self>) -> Self {
        range.into()
    }

    pub(crate) fn raw(start_inclusive: usize, end_inclusive: usize) -> Self {
        debug_assert!(start_inclusive <= end_inclusive);
        Self {
            start_inclusive,
            end_inclusive,
        }
    }

    /// Fewest number of values the argument accepts
    pub fn min_values(&self) -> usize {
        self.start_inclusive
    }

    /// Most number of values the argument accepts
    pub fn max_values(&self) -> usize {
        self.end_inclusive
    }

    /// Report whether the argument takes any values (ie is a flag)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::ValueRange;
    /// let range = ValueRange::new(5);
    /// assert!(range.takes_values());
    ///
    /// let range = ValueRange::new(0);
    /// assert!(!range.takes_values());
    /// ```
    pub fn takes_values(&self) -> bool {
        self.end_inclusive != 0
    }

    pub(crate) fn is_unbounded(&self) -> bool {
        self.end_inclusive == usize::MAX
    }

    pub(crate) fn is_fixed(&self) -> bool {
        self.start_inclusive == self.end_inclusive
    }

    pub(crate) fn is_multiple(&self) -> bool {
        self.start_inclusive != self.end_inclusive || 1 < self.start_inclusive
    }

    pub(crate) fn num_values(&self) -> Option<usize> {
        self.is_fixed().then_some(self.start_inclusive)
    }

    pub(crate) fn accepts_more(&self, current: usize) -> bool {
        current < self.end_inclusive
    }
}

impl std::ops::RangeBounds<usize> for ValueRange {
    fn start_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Included(&self.start_inclusive)
    }

    fn end_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Included(&self.end_inclusive)
    }
}

impl Default for ValueRange {
    fn default() -> Self {
        Self::SINGLE
    }
}

impl From<usize> for ValueRange {
    fn from(fixed: usize) -> Self {
        (fixed..=fixed).into()
    }
}

impl From<std::ops::Range<usize>> for ValueRange {
    fn from(range: std::ops::Range<usize>) -> Self {
        let start_inclusive = range.start;
        let end_inclusive = range.end.saturating_sub(1);
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<std::ops::RangeFull> for ValueRange {
    fn from(_: std::ops::RangeFull) -> Self {
        Self::FULL
    }
}

impl From<std::ops::RangeFrom<usize>> for ValueRange {
    fn from(range: std::ops::RangeFrom<usize>) -> Self {
        let start_inclusive = range.start;
        let end_inclusive = usize::MAX;
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<std::ops::RangeTo<usize>> for ValueRange {
    fn from(range: std::ops::RangeTo<usize>) -> Self {
        let start_inclusive = 0;
        let end_inclusive = range.end.saturating_sub(1);
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<std::ops::RangeInclusive<usize>> for ValueRange {
    fn from(range: std::ops::RangeInclusive<usize>) -> Self {
        let start_inclusive = *range.start();
        let end_inclusive = *range.end();
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl From<std::ops::RangeToInclusive<usize>> for ValueRange {
    fn from(range: std::ops::RangeToInclusive<usize>) -> Self {
        let start_inclusive = 0;
        let end_inclusive = range.end;
        Self::raw(start_inclusive, end_inclusive)
    }
}

impl std::fmt::Display for ValueRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ok!(self.start_inclusive.fmt(f));
        if self.is_fixed() {
        } else if self.end_inclusive == usize::MAX {
            ok!("..".fmt(f));
        } else {
            ok!("..=".fmt(f));
            ok!(self.end_inclusive.fmt(f));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ValueRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::ops::RangeBounds;

    #[test]
    fn from_fixed() {
        let range: ValueRange = 5.into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&5));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&5));
        assert!(range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), Some(5));
        assert!(range.takes_values());
    }

    #[test]
    fn from_fixed_empty() {
        let range: ValueRange = 0.into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&0));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&0));
        assert!(range.is_fixed());
        assert!(!range.is_multiple());
        assert_eq!(range.num_values(), Some(0));
        assert!(!range.takes_values());
    }

    #[test]
    fn from_range() {
        let range: ValueRange = (5..10).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&5));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&9));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }

    #[test]
    fn from_range_inclusive() {
        let range: ValueRange = (5..=10).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&5));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&10));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }

    #[test]
    fn from_range_full() {
        let range: ValueRange = (..).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&0));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&usize::MAX));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }

    #[test]
    fn from_range_from() {
        let range: ValueRange = (5..).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&5));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&usize::MAX));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }

    #[test]
    fn from_range_to() {
        let range: ValueRange = (..10).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&0));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&9));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }

    #[test]
    fn from_range_to_inclusive() {
        let range: ValueRange = (..=10).into();
        assert_eq!(range.start_bound(), std::ops::Bound::Included(&0));
        assert_eq!(range.end_bound(), std::ops::Bound::Included(&10));
        assert!(!range.is_fixed());
        assert!(range.is_multiple());
        assert_eq!(range.num_values(), None);
        assert!(range.takes_values());
    }
}
