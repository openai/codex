use core::{
    fmt::{self, Debug},
    iter::FusedIterator,
};

use crate::Weekday;

/// A collection of [`Weekday`]s stored as a single byte.
///
/// This type is `Copy` and provides efficient set-like and slice-like operations.
/// Many operations are `const` as well.
///
/// Implemented as a bitmask where bits 1-7 correspond to Monday-Sunday.
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeekdaySet(u8); // Invariant: the 8-th bit is always 0.

impl WeekdaySet {
    /// Create a `WeekdaySet` from an array of [`Weekday`]s.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::EMPTY, WeekdaySet::from_array([]));
    /// assert_eq!(WeekdaySet::single(Mon), WeekdaySet::from_array([Mon]));
    /// assert_eq!(WeekdaySet::ALL, WeekdaySet::from_array([Mon, Tue, Wed, Thu, Fri, Sat, Sun]));
    /// ```
    pub const fn from_array<const C: usize>(days: [Weekday; C]) -> Self {
        let mut acc = Self::EMPTY;
        let mut idx = 0;
        while idx < days.len() {
            acc.0 |= Self::single(days[idx]).0;
            idx += 1;
        }
        acc
    }

    /// Create a `WeekdaySet` from a single [`Weekday`].
    pub const fn single(weekday: Weekday) -> Self {
        match weekday {
            Weekday::Mon => Self(0b000_0001),
            Weekday::Tue => Self(0b000_0010),
            Weekday::Wed => Self(0b000_0100),
            Weekday::Thu => Self(0b000_1000),
            Weekday::Fri => Self(0b001_0000),
            Weekday::Sat => Self(0b010_0000),
            Weekday::Sun => Self(0b100_0000),
        }
    }

    /// Returns `Some(day)` if this collection contains exactly one day.
    ///
    /// Returns `None` otherwise.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).single_day(), Some(Mon));
    /// assert_eq!(WeekdaySet::from_array([Mon, Tue]).single_day(), None);
    /// assert_eq!(WeekdaySet::EMPTY.single_day(), None);
    /// assert_eq!(WeekdaySet::ALL.single_day(), None);
    /// ```
    pub const fn single_day(self) -> Option<Weekday> {
        match self {
            Self(0b000_0001) => Some(Weekday::Mon),
            Self(0b000_0010) => Some(Weekday::Tue),
            Self(0b000_0100) => Some(Weekday::Wed),
            Self(0b000_1000) => Some(Weekday::Thu),
            Self(0b001_0000) => Some(Weekday::Fri),
            Self(0b010_0000) => Some(Weekday::Sat),
            Self(0b100_0000) => Some(Weekday::Sun),
            _ => None,
        }
    }

    /// Adds a day to the collection.
    ///
    /// Returns `true` if the day was new to the collection.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// let mut weekdays = WeekdaySet::single(Mon);
    /// assert!(weekdays.insert(Tue));
    /// assert!(!weekdays.insert(Tue));
    /// ```
    pub fn insert(&mut self, day: Weekday) -> bool {
        if self.contains(day) {
            return false;
        }

        self.0 |= Self::single(day).0;
        true
    }

    /// Removes a day from the collection.
    ///
    /// Returns `true` if the collection did contain the day.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// let mut weekdays = WeekdaySet::single(Mon);
    /// assert!(weekdays.remove(Mon));
    /// assert!(!weekdays.remove(Mon));
    /// ```
    pub fn remove(&mut self, day: Weekday) -> bool {
        if self.contains(day) {
            self.0 &= !Self::single(day).0;
            return true;
        }

        false
    }

    /// Returns `true` if `other` contains all days in `self`.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert!(WeekdaySet::single(Mon).is_subset(WeekdaySet::ALL));
    /// assert!(!WeekdaySet::single(Mon).is_subset(WeekdaySet::EMPTY));
    /// assert!(WeekdaySet::EMPTY.is_subset(WeekdaySet::single(Mon)));
    /// ```
    pub const fn is_subset(self, other: Self) -> bool {
        self.intersection(other).0 == self.0
    }

    /// Returns days that are in both `self` and `other`.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).intersection(WeekdaySet::single(Mon)), WeekdaySet::single(Mon));
    /// assert_eq!(WeekdaySet::single(Mon).intersection(WeekdaySet::single(Tue)), WeekdaySet::EMPTY);
    /// assert_eq!(WeekdaySet::ALL.intersection(WeekdaySet::single(Mon)), WeekdaySet::single(Mon));
    /// assert_eq!(WeekdaySet::ALL.intersection(WeekdaySet::EMPTY), WeekdaySet::EMPTY);
    /// ```
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns days that are in either `self` or `other`.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).union(WeekdaySet::single(Mon)), WeekdaySet::single(Mon));
    /// assert_eq!(WeekdaySet::single(Mon).union(WeekdaySet::single(Tue)), WeekdaySet::from_array([Mon, Tue]));
    /// assert_eq!(WeekdaySet::ALL.union(WeekdaySet::single(Mon)), WeekdaySet::ALL);
    /// assert_eq!(WeekdaySet::ALL.union(WeekdaySet::EMPTY), WeekdaySet::ALL);
    /// ```
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns days that are in `self` or `other` but not in both.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).symmetric_difference(WeekdaySet::single(Mon)), WeekdaySet::EMPTY);
    /// assert_eq!(WeekdaySet::single(Mon).symmetric_difference(WeekdaySet::single(Tue)), WeekdaySet::from_array([Mon, Tue]));
    /// assert_eq!(
    ///     WeekdaySet::ALL.symmetric_difference(WeekdaySet::single(Mon)),
    ///     WeekdaySet::from_array([Tue, Wed, Thu, Fri, Sat, Sun]),
    /// );
    /// assert_eq!(WeekdaySet::ALL.symmetric_difference(WeekdaySet::EMPTY), WeekdaySet::ALL);
    /// ```
    pub const fn symmetric_difference(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }

    /// Returns days that are in `self` but not in `other`.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).difference(WeekdaySet::single(Mon)), WeekdaySet::EMPTY);
    /// assert_eq!(WeekdaySet::single(Mon).difference(WeekdaySet::single(Tue)), WeekdaySet::single(Mon));
    /// assert_eq!(WeekdaySet::EMPTY.difference(WeekdaySet::single(Mon)), WeekdaySet::EMPTY);
    /// ```
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Get the first day in the collection, starting from Monday.
    ///
    /// Returns `None` if the collection is empty.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).first(), Some(Mon));
    /// assert_eq!(WeekdaySet::single(Tue).first(), Some(Tue));
    /// assert_eq!(WeekdaySet::ALL.first(), Some(Mon));
    /// assert_eq!(WeekdaySet::EMPTY.first(), None);
    /// ```
    pub const fn first(self) -> Option<Weekday> {
        if self.is_empty() {
            return None;
        }

        // Find the first non-zero bit.
        let bit = 1 << self.0.trailing_zeros();

        Self(bit).single_day()
    }

    /// Get the last day in the collection, starting from Sunday.
    ///
    /// Returns `None` if the collection is empty.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).last(), Some(Mon));
    /// assert_eq!(WeekdaySet::single(Sun).last(), Some(Sun));
    /// assert_eq!(WeekdaySet::from_array([Mon, Tue]).last(), Some(Tue));
    /// assert_eq!(WeekdaySet::EMPTY.last(), None);
    /// ```
    pub fn last(self) -> Option<Weekday> {
        if self.is_empty() {
            return None;
        }

        // Find the last non-zero bit.
        let bit = 1 << (7 - self.0.leading_zeros());

        Self(bit).single_day()
    }

    /// Split the collection in two at the given day.
    ///
    /// Returns a tuple `(before, after)`. `before` contains all days starting from Monday
    /// up to but __not__ including `weekday`. `after` contains all days starting from `weekday`
    /// up to and including Sunday.
    const fn split_at(self, weekday: Weekday) -> (Self, Self) {
        let days_after = 0b1000_0000 - Self::single(weekday).0;
        let days_before = days_after ^ 0b0111_1111;
        (Self(self.0 & days_before), Self(self.0 & days_after))
    }

    /// Iterate over the [`Weekday`]s in the collection starting from a given day.
    ///
    /// Wraps around from Sunday to Monday if necessary.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// let weekdays = WeekdaySet::from_array([Mon, Wed, Fri]);
    /// let mut iter = weekdays.iter(Wed);
    /// assert_eq!(iter.next(), Some(Wed));
    /// assert_eq!(iter.next(), Some(Fri));
    /// assert_eq!(iter.next(), Some(Mon));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub const fn iter(self, start: Weekday) -> WeekdaySetIter {
        WeekdaySetIter { days: self, start }
    }

    /// Returns `true` if the collection contains the given day.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert!(WeekdaySet::single(Mon).contains(Mon));
    /// assert!(WeekdaySet::from_array([Mon, Tue]).contains(Tue));
    /// assert!(!WeekdaySet::single(Mon).contains(Tue));
    /// ```
    pub const fn contains(self, day: Weekday) -> bool {
        self.0 & Self::single(day).0 != 0
    }

    /// Returns `true` if the collection is empty.
    ///
    /// # Example
    /// ```
    /// # use chrono::{Weekday, WeekdaySet};
    /// assert!(WeekdaySet::EMPTY.is_empty());
    /// assert!(!WeekdaySet::single(Weekday::Mon).is_empty());
    /// ```
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// Returns the number of days in the collection.
    ///
    /// # Example
    /// ```
    /// # use chrono::WeekdaySet;
    /// use chrono::Weekday::*;
    /// assert_eq!(WeekdaySet::single(Mon).len(), 1);
    /// assert_eq!(WeekdaySet::from_array([Mon, Wed, Fri]).len(), 3);
    /// assert_eq!(WeekdaySet::ALL.len(), 7);
    /// ```
    pub const fn len(self) -> u8 {
        self.0.count_ones() as u8
    }

    /// An empty `WeekdaySet`.
    pub const EMPTY: Self = Self(0b000_0000);
    /// A `WeekdaySet` containing all seven `Weekday`s.
    pub const ALL: Self = Self(0b111_1111);
}

/// Print the underlying bitmask, padded to 7 bits.
///
/// # Example
/// ```
/// # use chrono::WeekdaySet;
/// use chrono::Weekday::*;
/// assert_eq!(format!("{:?}", WeekdaySet::single(Mon)), "WeekdaySet(0000001)");
/// assert_eq!(format!("{:?}", WeekdaySet::single(Tue)), "WeekdaySet(0000010)");
/// assert_eq!(format!("{:?}", WeekdaySet::ALL), "WeekdaySet(1111111)");
/// ```
impl Debug for WeekdaySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WeekdaySet({:0>7b})", self.0)
    }
}

/// An iterator over a collection of weekdays, starting from a given day.
///
/// See [`WeekdaySet::iter()`].
#[derive(Debug, Clone)]
pub struct WeekdaySetIter {
    days: WeekdaySet,
    start: Weekday,
}

impl Iterator for WeekdaySetIter {
    type Item = Weekday;

    fn next(&mut self) -> Option<Self::Item> {
        if self.days.is_empty() {
            return None;
        }

        // Split the collection in two at `start`.
        // Look for the first day among the days after `start` first, including `start` itself.
        // If there are no days after `start`, look for the first day among the days before `start`.
        let (before, after) = self.days.split_at(self.start);
        let days = if after.is_empty() { before } else { after };

        let next = days.first().expect("the collection is not empty");
        self.days.remove(next);
        Some(next)
    }
}

impl DoubleEndedIterator for WeekdaySetIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.days.is_empty() {
            return None;
        }

        // Split the collection in two at `start`.
        // Look for the last day among the days before `start` first, NOT including `start` itself.
        // If there are no days before `start`, look for the last day among the days after `start`.
        let (before, after) = self.days.split_at(self.start);
        let days = if before.is_empty() { after } else { before };

        let next_back = days.last().expect("the collection is not empty");
        self.days.remove(next_back);
        Some(next_back)
    }
}

impl ExactSizeIterator for WeekdaySetIter {
    fn len(&self) -> usize {
        self.days.len().into()
    }
}

impl FusedIterator for WeekdaySetIter {}

/// Print the collection as a slice-like list of weekdays.
///
/// # Example
/// ```
/// # use chrono::WeekdaySet;
/// use chrono::Weekday::*;
/// assert_eq!("[]", WeekdaySet::EMPTY.to_string());
/// assert_eq!("[Mon]", WeekdaySet::single(Mon).to_string());
/// assert_eq!("[Mon, Fri, Sun]", WeekdaySet::from_array([Mon, Fri, Sun]).to_string());
/// ```
impl fmt::Display for WeekdaySet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "[")?;
        let mut iter = self.iter(Weekday::Mon);
        if let Some(first) = iter.next() {
            write!(f, "{first}")?;
        }
        for weekday in iter {
            write!(f, ", {weekday}")?;
        }
        write!(f, "]")
    }
}

impl FromIterator<Weekday> for WeekdaySet {
    fn from_iter<T: IntoIterator<Item = Weekday>>(iter: T) -> Self {
        iter.into_iter().map(Self::single).fold(Self::EMPTY, Self::union)
    }
}

#[cfg(test)]
mod tests {
    use crate::Weekday;

    use super::WeekdaySet;

    impl WeekdaySet {
        /// Iterate over all 128 possible sets, from `EMPTY` to `ALL`.
        fn iter_all() -> impl Iterator<Item = Self> {
            (0b0000_0000..0b1000_0000).map(Self)
        }
    }

    /// Panics if the 8-th bit of `self` is not 0.
    fn assert_8th_bit_invariant(days: WeekdaySet) {
        assert!(days.0 & 0b1000_0000 == 0, "the 8-th bit of {days:?} is not 0");
    }

    #[test]
    fn debug_prints_8th_bit_if_not_zero() {
        assert_eq!(format!("{:?}", WeekdaySet(0b1000_0000)), "WeekdaySet(10000000)");
    }

    #[test]
    fn bitwise_set_operations_preserve_8th_bit_invariant() {
        for set1 in WeekdaySet::iter_all() {
            for set2 in WeekdaySet::iter_all() {
                assert_8th_bit_invariant(set1.union(set2));
                assert_8th_bit_invariant(set1.intersection(set2));
                assert_8th_bit_invariant(set1.symmetric_difference(set2));
            }
        }
    }

    /// Test `split_at` on all possible arguments.
    #[test]
    fn split_at_is_equivalent_to_iterating() {
        use Weekday::*;

        // `split_at()` is used in `iter()`, so we must not iterate
        // over all days with `WeekdaySet::ALL.iter(Mon)`.
        const WEEK: [Weekday; 7] = [Mon, Tue, Wed, Thu, Fri, Sat, Sun];

        for weekdays in WeekdaySet::iter_all() {
            for split_day in WEEK {
                let expected_before: WeekdaySet = WEEK
                    .into_iter()
                    .take_while(|&day| day != split_day)
                    .filter(|&day| weekdays.contains(day))
                    .collect();
                let expected_after: WeekdaySet = WEEK
                    .into_iter()
                    .skip_while(|&day| day != split_day)
                    .filter(|&day| weekdays.contains(day))
                    .collect();

                assert_eq!(
                    (expected_before, expected_after),
                    weekdays.split_at(split_day),
                    "split_at({split_day}) failed for {weekdays}",
                );
            }
        }
    }
}
