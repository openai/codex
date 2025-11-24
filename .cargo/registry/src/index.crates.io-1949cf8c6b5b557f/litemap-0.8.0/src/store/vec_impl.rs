// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use alloc::vec::Vec;

type MapF<K, V> = fn(&(K, V)) -> (&K, &V);

#[inline]
fn map_f<K, V>(input: &(K, V)) -> (&K, &V) {
    (&input.0, &input.1)
}

type MapFMut<K, V> = fn(&mut (K, V)) -> (&K, &mut V);

#[inline]
fn map_f_mut<K, V>(input: &mut (K, V)) -> (&K, &mut V) {
    (&input.0, &mut input.1)
}

impl<K, V> StoreConstEmpty<K, V> for Vec<(K, V)> {
    const EMPTY: Vec<(K, V)> = Vec::new();
}

impl<K, V> Store<K, V> for Vec<(K, V)> {
    #[inline]
    fn lm_len(&self) -> usize {
        self.as_slice().len()
    }

    #[inline]
    fn lm_is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    #[inline]
    fn lm_get(&self, index: usize) -> Option<(&K, &V)> {
        self.as_slice().get(index).map(map_f)
    }

    #[inline]
    fn lm_last(&self) -> Option<(&K, &V)> {
        self.as_slice().last().map(map_f)
    }

    #[inline]
    fn lm_binary_search_by<F>(&self, mut cmp: F) -> Result<usize, usize>
    where
        F: FnMut(&K) -> Ordering,
    {
        self.as_slice().binary_search_by(|(k, _)| cmp(k))
    }
}

impl<K, V> StoreSlice<K, V> for Vec<(K, V)> {
    type Slice = [(K, V)];

    fn lm_get_range(&self, range: Range<usize>) -> Option<&Self::Slice> {
        self.get(range)
    }
}

impl<K, V> StoreMut<K, V> for Vec<(K, V)> {
    #[inline]
    fn lm_with_capacity(capacity: usize) -> Self {
        Self::with_capacity(capacity)
    }

    #[inline]
    fn lm_reserve(&mut self, additional: usize) {
        self.reserve(additional)
    }

    #[inline]
    fn lm_get_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.as_mut_slice().get_mut(index).map(map_f_mut)
    }

    #[inline]
    fn lm_push(&mut self, key: K, value: V) {
        self.push((key, value))
    }

    #[inline]
    fn lm_insert(&mut self, index: usize, key: K, value: V) {
        self.insert(index, (key, value))
    }

    #[inline]
    fn lm_remove(&mut self, index: usize) -> (K, V) {
        self.remove(index)
    }

    #[inline]
    fn lm_clear(&mut self) {
        self.clear()
    }
}

impl<K: Ord, V> StoreBulkMut<K, V> for Vec<(K, V)> {
    #[inline]
    fn lm_retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&K, &V) -> bool,
    {
        self.retain(|(k, v)| predicate(k, v))
    }

    /// Extends this store with items from an iterator.
    ///
    /// It uses a two-pass (sort + dedup) approach to avoid any potential quadratic costs.
    ///
    /// The asymptotic worst case complexity is O((n + m) log(n + m)), where `n`
    /// is the number of elements already in `self` and `m` is the number of elements
    /// in the iterator. The best case complexity is O(m), when the input iterator is
    /// already sorted, keys aren't duplicated and all keys sort after the existing ones.
    #[inline]
    fn lm_extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Ord,
    {
        // First N elements in self that are already sorted and not duplicated.
        let mut sorted_len = self.len();
        // Use Vec::extend as it has a specialized code for slice and trusted-len iterators.
        self.extend(iter);
        // `sorted_len` is the length of the sorted run before extension
        // window slice `w` is guaranteed to have a length of 2.
        #[allow(clippy::indexing_slicing)]
        {
            // Count new elements that are sorted and non-duplicated.
            // Starting from the end of the existing sorted run, if any.
            // Thus, start the slice at sorted_len.saturating_sub(1).
            sorted_len += self[sorted_len.saturating_sub(1)..]
                .windows(2)
                .take_while(|w| w[0].0 < w[1].0)
                .count();
        }
        // `windows(2)` only yields `slice len - 1` times, or none if the slice is empty.
        // In other words, the first extended element of the slice won't be counted as sorted
        // if self was initially empty (sorted_len == 0). We adjust this by adding 1 if the
        // original slice was empty but became not empty after extend.
        sorted_len += (sorted_len == 0 && !self.is_empty()) as usize;

        // If everything was in order, we're done
        if sorted_len >= self.len() {
            return;
        }

        // Use stable sort to keep relative order of duplicates.
        self.sort_by(|a, b| a.0.cmp(&b.0));
        // Deduplicate by keeping the last element of the run in the first slice.
        let (dedup, _merged_dup) = partition_dedup_by(self);
        sorted_len = dedup.len();
        self.truncate(sorted_len);
    }
}

/// Moves all but the _last_ of consecutive elements to the end of the slice satisfying
/// equality on K.
///
/// Returns two slices. The first contains no consecutive repeated elements.
/// The second contains all the duplicates in no specified order.
///
/// This is based on std::slice::partition_dedup_by (currently unstable) but retains the
/// _last_ element of the duplicate run in the first slice (instead of first).
#[inline]
#[allow(clippy::type_complexity)]
fn partition_dedup_by<K: Eq, V>(v: &mut [(K, V)]) -> (&mut [(K, V)], &mut [(K, V)]) {
    // Although we have a mutable reference to `self`, we cannot make
    // *arbitrary* changes. The comparison could panic, so we
    // must ensure that the slice is in a valid state at all times.
    //
    // The way that we handle this is by using swaps; we iterate
    // over all the elements, swapping as we go so that at the end
    // the elements we wish to keep are in the front, and those we
    // wish to reject are at the back. We can then split the slice.
    // This operation is still `O(n)`.
    //
    // Example:
    // Assume (K, V) is (char, u8):
    //
    // We start in this state, where `r` represents "next
    // read" and `w` represents "next_write".
    //
    //              r
    //     | a,0 | b,0 | b,1 | c,0 | d,0 | d,1 |
    //              w
    //
    // Comparing self[r] against self[w-1], this is not a duplicate, so
    // we swap self[r] and self[w] (no effect as r==w) and then increment both
    // r and w, leaving us with:
    //
    //                    r
    //     | a,0 | b,0 | b,1 | c,0 | d,0 | d,0 |
    //                    w
    //
    // Comparing self[r] against self[w-1], this value is a duplicate,
    // we swap self[r] and self[w-1] and then increment `r`:
    //
    //                          r
    //     | a,0 | b,1 | b,0 | c,0 | d,0 | d,1 |
    //                    w
    //
    // Comparing self[r] against self[w-1], this is not a duplicate,
    // so swap self[r] and self[w] and advance r and w:
    //
    //                                r
    //     | a,0 | b,1 | c,0 | b,0 | d,0 | d,1 |
    //                          w
    //
    // Comparing self[r] against self[w-1], this is not a duplicate,
    // so swap self[r] and self[w] and advance r and w:
    //
    //                                      r
    //     | a,0 | b,1 | c,0 | d,0 | b,0 | d,1 |
    //                                w
    //
    // Comparing self[r] against self[w-1], this value is a duplicate,
    // we swap self[r] and self[w-1] and then increment `r`:
    //                                             r
    //     | a,0 | b,1 | c,0 | d,1 | b,0 | d,0 |
    //                                w
    //
    // End of slice, as r > len. Split at w.

    if v.len() <= 1 {
        return (v, &mut []);
    }

    let mut read_idx: usize = 1;
    let mut write_idx: usize = 1;

    while let Some((before_read, [read, ..])) = v.split_at_mut_checked(read_idx) {
        // First, `read_idx >= write_idx` is always true as `read_idx` is always incremented
        // whereas `write_idx` is only incremented when a distinct element is found.
        // Second, before_read is always at least 1 length due to read_idx being initialized to 1.
        // Thus it is safe to index before_read with `write_idx - 1`.
        #[allow(clippy::indexing_slicing)]
        let prev_write = &mut before_read[write_idx - 1];
        if read.0 == prev_write.0 {
            core::mem::swap(read, prev_write);
        } else {
            // Equivalent to checking if write_idx == read_idx
            if let Some(write) = before_read.get_mut(write_idx) {
                core::mem::swap(read, write);
            }
            write_idx += 1;
        }
        read_idx += 1;
    }
    v.split_at_mut(write_idx)
}

impl<K: Ord, V> StoreFromIterable<K, V> for Vec<(K, V)> {
    fn lm_sort_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut v = Self::new();
        v.lm_extend(iter);
        v
    }
}

impl<'a, K: 'a, V: 'a> StoreIterable<'a, K, V> for Vec<(K, V)> {
    type KeyValueIter = core::iter::Map<core::slice::Iter<'a, (K, V)>, MapF<K, V>>;

    #[inline]
    fn lm_iter(&'a self) -> Self::KeyValueIter {
        self.as_slice().iter().map(map_f)
    }
}

impl<'a, K: 'a, V: 'a> StoreIterableMut<'a, K, V> for Vec<(K, V)> {
    type KeyValueIterMut = core::iter::Map<core::slice::IterMut<'a, (K, V)>, MapFMut<K, V>>;

    #[inline]
    fn lm_iter_mut(&'a mut self) -> Self::KeyValueIterMut {
        self.as_mut_slice().iter_mut().map(map_f_mut)
    }
}

impl<K, V> StoreIntoIterator<K, V> for Vec<(K, V)> {
    type KeyValueIntoIter = alloc::vec::IntoIter<(K, V)>;

    #[inline]
    fn lm_into_iter(self) -> Self::KeyValueIntoIter {
        IntoIterator::into_iter(self)
    }

    #[inline]
    fn lm_extend_end(&mut self, other: Self) {
        self.extend(other)
    }

    #[inline]
    fn lm_extend_start(&mut self, other: Self) {
        self.splice(0..0, other);
    }
}

impl<K, V> StoreFromIterator<K, V> for Vec<(K, V)> {}

#[test]
fn test_vec_impl() {
    crate::testing::check_store_full::<Vec<(u32, u64)>>();
}
