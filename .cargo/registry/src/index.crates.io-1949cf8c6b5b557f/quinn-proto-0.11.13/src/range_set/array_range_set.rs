use std::ops::Range;

use tinyvec::TinyVec;

/// A set of u64 values optimized for long runs and random insert/delete/contains
///
/// `ArrayRangeSet` uses an array representation, where each array entry represents
/// a range.
///
/// The array-based RangeSet provides 2 benefits:
/// - There exists an inline representation, which avoids the need of heap
///   allocating ACK ranges for SentFrames for small ranges.
/// - Iterating over ranges should usually be faster since there is only
///   a single cache-friendly contiguous range.
///
/// `ArrayRangeSet` is especially useful for tracking ACK ranges where the amount
/// of ranges is usually very low (since ACK numbers are in consecutive fashion
/// unless reordering or packet loss occur).
#[derive(Debug, Default)]
pub struct ArrayRangeSet(TinyVec<[Range<u64>; ARRAY_RANGE_SET_INLINE_CAPACITY]>);

/// The capacity of elements directly stored in [`ArrayRangeSet`]
///
/// An inline capacity of 2 is chosen to keep `SentFrame` below 128 bytes.
const ARRAY_RANGE_SET_INLINE_CAPACITY: usize = 2;

impl Clone for ArrayRangeSet {
    fn clone(&self) -> Self {
        // tinyvec keeps the heap representation after clones.
        // We rather prefer the inline representation for clones if possible,
        // since clones (e.g. for storage in `SentFrames`) are rarely mutated
        if self.0.is_inline() || self.0.len() > ARRAY_RANGE_SET_INLINE_CAPACITY {
            return Self(self.0.clone());
        }

        let mut vec = TinyVec::new();
        vec.extend_from_slice(self.0.as_slice());
        Self(vec)
    }
}

impl ArrayRangeSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Range<u64>> + '_ {
        self.0.iter().cloned()
    }

    pub fn elts(&self) -> impl Iterator<Item = u64> + '_ {
        self.iter().flatten()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, x: u64) -> bool {
        for range in self.0.iter() {
            if range.start > x {
                // We only get here if there was no prior range that contained x
                return false;
            } else if range.contains(&x) {
                return true;
            }
        }
        false
    }

    pub fn subtract(&mut self, other: &Self) {
        // TODO: This can potentially be made more efficient, since the we know
        // individual ranges are not overlapping, and the next range must start
        // after the last one finished
        for range in &other.0 {
            self.remove(range.clone());
        }
    }

    pub fn insert_one(&mut self, x: u64) -> bool {
        self.insert(x..x + 1)
    }

    pub fn insert(&mut self, x: Range<u64>) -> bool {
        let mut result = false;

        if x.is_empty() {
            // Don't try to deal with ranges where x.end <= x.start
            return false;
        }

        let mut idx = 0;
        while idx != self.0.len() {
            let range = &mut self.0[idx];

            if range.start > x.end {
                // The range is fully before this range and therefore not extensible.
                // Add a new range to the left
                self.0.insert(idx, x);
                return true;
            } else if range.start > x.start {
                // The new range starts before this range but overlaps.
                // Extend the current range to the left
                // Note that we don't have to merge a potential left range, since
                // this case would have been captured by merging the right range
                // in the previous loop iteration
                result = true;
                range.start = x.start;
            }

            // At this point we have handled all parts of the new range which
            // are in front of the current range. Now we handle everything from
            // the start of the current range

            if x.end <= range.end {
                // Fully contained
                return result;
            } else if x.start <= range.end {
                // Extend the current range to the end of the new range.
                // Since it's not contained it must be bigger
                range.end = x.end;

                // Merge all follow-up ranges which overlap
                while idx != self.0.len() - 1 {
                    let curr = self.0[idx].clone();
                    let next = self.0[idx + 1].clone();
                    if curr.end >= next.start {
                        self.0[idx].end = next.end.max(curr.end);
                        self.0.remove(idx + 1);
                    } else {
                        break;
                    }
                }

                return true;
            }

            idx += 1;
        }

        // Insert a range at the end
        self.0.push(x);
        true
    }

    pub fn remove(&mut self, x: Range<u64>) -> bool {
        let mut result = false;

        if x.is_empty() {
            // Don't try to deal with ranges where x.end <= x.start
            return false;
        }

        let mut idx = 0;
        while idx != self.0.len() && x.start != x.end {
            let range = self.0[idx].clone();

            if x.end <= range.start {
                // The range is fully before this range
                return result;
            } else if x.start >= range.end {
                // The range is fully after this range
                idx += 1;
                continue;
            }

            // The range overlaps with this range
            result = true;

            let left = range.start..x.start;
            let right = x.end..range.end;
            if left.is_empty() && right.is_empty() {
                self.0.remove(idx);
            } else if left.is_empty() {
                self.0[idx] = right;
                idx += 1;
            } else if right.is_empty() {
                self.0[idx] = left;
                idx += 1;
            } else {
                self.0[idx] = right;
                self.0.insert(idx, left);
                idx += 2;
            }
        }

        result
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn pop_min(&mut self) -> Option<Range<u64>> {
        if !self.0.is_empty() {
            Some(self.0.remove(0))
        } else {
            None
        }
    }

    pub fn min(&self) -> Option<u64> {
        self.iter().next().map(|x| x.start)
    }

    pub fn max(&self) -> Option<u64> {
        self.iter().next_back().map(|x| x.end - 1)
    }
}
