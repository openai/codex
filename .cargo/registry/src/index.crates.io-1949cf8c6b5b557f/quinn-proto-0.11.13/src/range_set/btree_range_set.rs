use std::{
    cmp,
    cmp::Ordering,
    collections::{BTreeMap, btree_map},
    ops::{
        Bound::{Excluded, Included},
        Range,
    },
};

/// A set of u64 values optimized for long runs and random insert/delete/contains
#[derive(Debug, Default, Clone)]
pub struct RangeSet(BTreeMap<u64, u64>);

impl RangeSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn contains(&self, x: u64) -> bool {
        self.pred(x).is_some_and(|(_, end)| end > x)
    }

    pub fn insert_one(&mut self, x: u64) -> bool {
        if let Some((start, end)) = self.pred(x) {
            match end.cmp(&x) {
                // Wholly contained
                Ordering::Greater => {
                    return false;
                }
                Ordering::Equal => {
                    // Extend existing
                    self.0.remove(&start);
                    let mut new_end = x + 1;
                    if let Some((next_start, next_end)) = self.succ(x) {
                        if next_start == new_end {
                            self.0.remove(&next_start);
                            new_end = next_end;
                        }
                    }
                    self.0.insert(start, new_end);
                    return true;
                }
                _ => {}
            }
        }
        let mut new_end = x + 1;
        if let Some((next_start, next_end)) = self.succ(x) {
            if next_start == new_end {
                self.0.remove(&next_start);
                new_end = next_end;
            }
        }
        self.0.insert(x, new_end);
        true
    }

    pub fn insert(&mut self, mut x: Range<u64>) -> bool {
        if x.is_empty() {
            return false;
        }
        if let Some((start, end)) = self.pred(x.start) {
            if end >= x.end {
                // Wholly contained
                return false;
            } else if end >= x.start {
                // Extend overlapping predecessor
                self.0.remove(&start);
                x.start = start;
            }
        }
        while let Some((next_start, next_end)) = self.succ(x.start) {
            if next_start > x.end {
                break;
            }
            // Overlaps with successor
            self.0.remove(&next_start);
            x.end = cmp::max(next_end, x.end);
        }
        self.0.insert(x.start, x.end);
        true
    }

    /// Find closest range to `x` that begins at or before it
    fn pred(&self, x: u64) -> Option<(u64, u64)> {
        self.0
            .range((Included(0), Included(x)))
            .next_back()
            .map(|(&x, &y)| (x, y))
    }

    /// Find the closest range to `x` that begins after it
    fn succ(&self, x: u64) -> Option<(u64, u64)> {
        self.0
            .range((Excluded(x), Included(u64::MAX)))
            .next()
            .map(|(&x, &y)| (x, y))
    }

    pub fn remove(&mut self, x: Range<u64>) -> bool {
        if x.is_empty() {
            return false;
        }

        let before = match self.pred(x.start) {
            Some((start, end)) if end > x.start => {
                self.0.remove(&start);
                if start < x.start {
                    self.0.insert(start, x.start);
                }
                if end > x.end {
                    self.0.insert(x.end, end);
                }
                // Short-circuit if we cannot possibly overlap with another range
                if end >= x.end {
                    return true;
                }
                true
            }
            Some(_) | None => false,
        };
        let mut after = false;
        while let Some((start, end)) = self.succ(x.start) {
            if start >= x.end {
                break;
            }
            after = true;
            self.0.remove(&start);
            if end > x.end {
                self.0.insert(x.end, end);
                break;
            }
        }
        before || after
    }

    /// Add a range to the set, returning the intersection of current ranges with the new one
    pub fn replace(&mut self, mut range: Range<u64>) -> Replace<'_> {
        let pred = if let Some((prev_start, prev_end)) = self
            .pred(range.start)
            .filter(|&(_, end)| end >= range.start)
        {
            self.0.remove(&prev_start);
            let replaced_start = range.start;
            range.start = range.start.min(prev_start);
            let replaced_end = range.end.min(prev_end);
            range.end = range.end.max(prev_end);
            if replaced_start != replaced_end {
                Some(replaced_start..replaced_end)
            } else {
                None
            }
        } else {
            None
        };
        Replace {
            set: self,
            range,
            pred,
        }
    }

    pub fn add(&mut self, other: &Self) {
        for (&start, &end) in &other.0 {
            self.insert(start..end);
        }
    }

    pub fn subtract(&mut self, other: &Self) {
        for (&start, &end) in &other.0 {
            self.remove(start..end);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn min(&self) -> Option<u64> {
        self.0.first_key_value().map(|(&start, _)| start)
    }

    pub fn max(&self) -> Option<u64> {
        self.0.last_key_value().map(|(_, &end)| end - 1)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter())
    }
    pub fn elts(&self) -> EltIter<'_> {
        EltIter {
            inner: self.0.iter(),
            next: 0,
            end: 0,
        }
    }

    pub fn peek_min(&self) -> Option<Range<u64>> {
        let (&start, &end) = self.0.iter().next()?;
        Some(start..end)
    }

    pub fn pop_min(&mut self) -> Option<Range<u64>> {
        let result = self.peek_min()?;
        self.0.remove(&result.start);
        Some(result)
    }
}

pub struct Iter<'a>(btree_map::Iter<'a, u64, u64>);

impl Iterator for Iter<'_> {
    type Item = Range<u64>;
    fn next(&mut self) -> Option<Range<u64>> {
        let (&start, &end) = self.0.next()?;
        Some(start..end)
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Range<u64>> {
        let (&start, &end) = self.0.next_back()?;
        Some(start..end)
    }
}

impl<'a> IntoIterator for &'a RangeSet {
    type Item = Range<u64>;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

pub struct EltIter<'a> {
    inner: btree_map::Iter<'a, u64, u64>,
    next: u64,
    end: u64,
}

impl Iterator for EltIter<'_> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.next == self.end {
            let (&start, &end) = self.inner.next()?;
            self.next = start;
            self.end = end;
        }
        let x = self.next;
        self.next += 1;
        Some(x)
    }
}

impl DoubleEndedIterator for EltIter<'_> {
    fn next_back(&mut self) -> Option<u64> {
        if self.next == self.end {
            let (&start, &end) = self.inner.next_back()?;
            self.next = start;
            self.end = end;
        }
        self.end -= 1;
        Some(self.end)
    }
}

/// Iterator returned by `RangeSet::replace`
pub struct Replace<'a> {
    set: &'a mut RangeSet,
    /// Portion of the intersection arising from a range beginning at or before the newly inserted
    /// range
    pred: Option<Range<u64>>,
    /// Union of the input range and all ranges that have been visited by the iterator so far
    range: Range<u64>,
}

impl Iterator for Replace<'_> {
    type Item = Range<u64>;
    fn next(&mut self) -> Option<Range<u64>> {
        if let Some(pred) = self.pred.take() {
            // If a range starting before the inserted range overlapped with it, return the
            // corresponding overlap first
            return Some(pred);
        }

        let (next_start, next_end) = self.set.succ(self.range.start)?;
        if next_start > self.range.end {
            // If the next successor range starts after the current range ends, there can be no more
            // overlaps. This is sound even when `self.range.end` is increased because `RangeSet` is
            // guaranteed not to contain pairs of ranges that could be simplified.
            return None;
        }
        // Remove the redundant range...
        self.set.0.remove(&next_start);
        // ...and handle the case where the redundant range ends later than the new range.
        let replaced_end = self.range.end.min(next_end);
        self.range.end = self.range.end.max(next_end);
        if next_start == replaced_end {
            // If the redundant range started exactly where the new range ended, there was no
            // overlap with it or any later range.
            None
        } else {
            Some(next_start..replaced_end)
        }
    }
}

impl Drop for Replace<'_> {
    fn drop(&mut self) {
        // Ensure we drain all remaining overlapping ranges
        for _ in &mut *self {}
        // Insert the final aggregate range
        self.set.0.insert(self.range.start, self.range.end);
    }
}

/// This module contains tests which only apply for this `RangeSet` implementation
///
/// Tests which apply for all implementations can be found in the `tests.rs` module
#[cfg(test)]
mod tests {
    #![allow(clippy::single_range_in_vec_init)] // https://github.com/rust-lang/rust-clippy/issues/11086
    use super::*;

    #[test]
    fn replace_contained() {
        let mut set = RangeSet::new();
        set.insert(2..4);
        assert_eq!(set.replace(1..5).collect::<Vec<_>>(), &[2..4]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 1..5);
    }

    #[test]
    fn replace_contains() {
        let mut set = RangeSet::new();
        set.insert(1..5);
        assert_eq!(set.replace(2..4).collect::<Vec<_>>(), &[2..4]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 1..5);
    }

    #[test]
    fn replace_pred() {
        let mut set = RangeSet::new();
        set.insert(2..4);
        assert_eq!(set.replace(3..5).collect::<Vec<_>>(), &[3..4]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 2..5);
    }

    #[test]
    fn replace_succ() {
        let mut set = RangeSet::new();
        set.insert(2..4);
        assert_eq!(set.replace(1..3).collect::<Vec<_>>(), &[2..3]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 1..4);
    }

    #[test]
    fn replace_exact_pred() {
        let mut set = RangeSet::new();
        set.insert(2..4);
        assert_eq!(set.replace(4..6).collect::<Vec<_>>(), &[]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 2..6);
    }

    #[test]
    fn replace_exact_succ() {
        let mut set = RangeSet::new();
        set.insert(2..4);
        assert_eq!(set.replace(0..2).collect::<Vec<_>>(), &[]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.peek_min().unwrap(), 0..4);
    }
}
