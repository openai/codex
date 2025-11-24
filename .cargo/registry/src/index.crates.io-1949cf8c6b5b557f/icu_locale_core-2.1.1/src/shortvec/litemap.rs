// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::ShortBoxSlice;
use super::ShortBoxSliceInner;
#[cfg(feature = "alloc")]
use super::ShortBoxSliceIntoIter;
use litemap::store::*;

impl<K, V> StoreConstEmpty<K, V> for ShortBoxSlice<(K, V)> {
    const EMPTY: ShortBoxSlice<(K, V)> = ShortBoxSlice::new();
}

impl<K, V> StoreSlice<K, V> for ShortBoxSlice<(K, V)> {
    type Slice = [(K, V)];

    #[inline]
    fn lm_get_range(&self, range: core::ops::Range<usize>) -> Option<&Self::Slice> {
        self.get(range)
    }
}

impl<K, V> Store<K, V> for ShortBoxSlice<(K, V)> {
    #[inline]
    fn lm_len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn lm_is_empty(&self) -> bool {
        use ShortBoxSliceInner::*;
        matches!(self.0, ZeroOne(None))
    }

    #[inline]
    fn lm_get(&self, index: usize) -> Option<(&K, &V)> {
        self.get(index).map(|elt| (&elt.0, &elt.1))
    }

    #[inline]
    fn lm_last(&self) -> Option<(&K, &V)> {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(ref v) => v.as_ref(),
            #[cfg(feature = "alloc")]
            Multi(ref v) => v.last(),
            #[cfg(not(feature = "alloc"))]
            Two([_, ref v]) => Some(v),
        }
        .map(|elt| (&elt.0, &elt.1))
    }

    #[inline]
    fn lm_binary_search_by<F>(&self, mut cmp: F) -> Result<usize, usize>
    where
        F: FnMut(&K) -> core::cmp::Ordering,
    {
        self.binary_search_by(|(k, _)| cmp(k))
    }
}

#[cfg(feature = "alloc")]
impl<K: Ord, V> StoreFromIterable<K, V> for ShortBoxSlice<(K, V)> {
    fn lm_sort_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        alloc::vec::Vec::lm_sort_from_iter(iter).into()
    }
}

#[cfg(feature = "alloc")]
impl<K, V> StoreMut<K, V> for ShortBoxSlice<(K, V)> {
    fn lm_with_capacity(_capacity: usize) -> Self {
        ShortBoxSlice::new()
    }

    fn lm_reserve(&mut self, _additional: usize) {}

    fn lm_get_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.get_mut(index).map(|elt| (&elt.0, &mut elt.1))
    }

    fn lm_push(&mut self, key: K, value: V) {
        self.push((key, value))
    }

    fn lm_insert(&mut self, index: usize, key: K, value: V) {
        self.insert(index, (key, value))
    }

    fn lm_remove(&mut self, index: usize) -> (K, V) {
        self.remove(index)
    }

    fn lm_clear(&mut self) {
        self.clear();
    }
}

#[cfg(feature = "alloc")]
impl<K: Ord, V> StoreBulkMut<K, V> for ShortBoxSlice<(K, V)> {
    fn lm_retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&K, &V) -> bool,
    {
        self.retain(|(k, v)| predicate(k, v))
    }

    fn lm_extend<I>(&mut self, other: I)
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut other = other.into_iter();
        // Use an Option to hold the first item of the map and move it to
        // items if there are more items. Meaning that if items is not
        // empty, first is None.
        let mut first = None;
        let mut items = alloc::vec::Vec::new();
        match core::mem::take(&mut self.0) {
            ShortBoxSliceInner::ZeroOne(zo) => {
                first = zo;
                // Attempt to avoid the items allocation by advancing the iterator
                // up to two times. If we eventually find a second item, we can
                // lm_extend the Vec and with the first, next (second) and the rest
                // of the iterator.
                while let Some(next) = other.next() {
                    if let Some(first) = first.take() {
                        // lm_extend will take care of sorting and deduplicating
                        // first, next and the rest of the other iterator.
                        items.lm_extend([first, next].into_iter().chain(other));
                        break;
                    }
                    first = Some(next);
                }
            }
            ShortBoxSliceInner::Multi(existing_items) => {
                items.reserve_exact(existing_items.len() + other.size_hint().0);
                // We use a plain extend with existing items, which are already valid and
                // lm_extend will fold over rest of the iterator sorting and deduplicating as needed.
                items.extend(existing_items);
                items.lm_extend(other);
            }
        }
        if items.is_empty() {
            debug_assert!(items.is_empty());
            self.0 = ShortBoxSliceInner::ZeroOne(first);
        } else {
            debug_assert!(first.is_none());
            self.0 = ShortBoxSliceInner::Multi(items.into_boxed_slice());
        }
    }
}

impl<'a, K: 'a, V: 'a> StoreIterable<'a, K, V> for ShortBoxSlice<(K, V)> {
    type KeyValueIter =
        core::iter::Map<core::slice::Iter<'a, (K, V)>, for<'r> fn(&'r (K, V)) -> (&'r K, &'r V)>;

    fn lm_iter(&'a self) -> Self::KeyValueIter {
        self.iter().map(|elt| (&elt.0, &elt.1))
    }
}

#[cfg(feature = "alloc")]
impl<K, V> StoreFromIterator<K, V> for ShortBoxSlice<(K, V)> {}

#[cfg(feature = "alloc")]
impl<'a, K: 'a, V: 'a> StoreIterableMut<'a, K, V> for ShortBoxSlice<(K, V)> {
    type KeyValueIterMut = core::iter::Map<
        core::slice::IterMut<'a, (K, V)>,
        for<'r> fn(&'r mut (K, V)) -> (&'r K, &'r mut V),
    >;

    fn lm_iter_mut(
        &'a mut self,
    ) -> <Self as litemap::store::StoreIterableMut<'a, K, V>>::KeyValueIterMut {
        self.iter_mut().map(|elt| (&elt.0, &mut elt.1))
    }
}

#[cfg(feature = "alloc")]
impl<K, V> StoreIntoIterator<K, V> for ShortBoxSlice<(K, V)> {
    type KeyValueIntoIter = ShortBoxSliceIntoIter<(K, V)>;

    fn lm_into_iter(self) -> Self::KeyValueIntoIter {
        self.into_iter()
    }

    // leave lm_extend_end as default

    // leave lm_extend_start as default
}

#[test]
fn test_short_slice_impl() {
    litemap::testing::check_store::<ShortBoxSlice<(u32, u64)>>();
}

#[test]
fn test_short_slice_impl_full() {
    litemap::testing::check_store_full::<ShortBoxSlice<(u32, u64)>>();
}
