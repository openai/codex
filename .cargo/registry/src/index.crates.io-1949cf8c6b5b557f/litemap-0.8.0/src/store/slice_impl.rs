// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;

type MapF<K, V> = fn(&(K, V)) -> (&K, &V);

#[inline]
fn map_f<K, V>(input: &(K, V)) -> (&K, &V) {
    (&input.0, &input.1)
}

impl<'a, K: 'a, V: 'a> StoreConstEmpty<K, V> for &'a [(K, V)] {
    const EMPTY: &'a [(K, V)] = &[];
}

impl<'a, K: 'a, V: 'a> Store<K, V> for &'a [(K, V)] {
    #[inline]
    fn lm_len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn lm_is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline]
    fn lm_get(&self, index: usize) -> Option<(&K, &V)> {
        self.get(index).map(map_f)
    }

    #[inline]
    fn lm_last(&self) -> Option<(&K, &V)> {
        self.last().map(map_f)
    }

    #[inline]
    fn lm_binary_search_by<F>(&self, mut cmp: F) -> Result<usize, usize>
    where
        F: FnMut(&K) -> Ordering,
    {
        self.binary_search_by(|(k, _)| cmp(k))
    }
}

impl<K, V> StoreSlice<K, V> for &[(K, V)] {
    type Slice = [(K, V)];

    fn lm_get_range(&self, range: Range<usize>) -> Option<&Self::Slice> {
        self.get(range)
    }
}

impl<'a, K: 'a, V: 'a> StoreIterable<'a, K, V> for &'a [(K, V)] {
    type KeyValueIter = core::iter::Map<core::slice::Iter<'a, (K, V)>, MapF<K, V>>;

    #[inline]
    fn lm_iter(&'a self) -> Self::KeyValueIter {
        self.iter().map(map_f)
    }
}
