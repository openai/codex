use super::{
    Bucket, IndexMap, IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
use crate::util::{slice_eq, try_simplify_range};
use crate::GetDisjointMutError;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{self, Bound, Index, IndexMut, RangeBounds};

/// A dynamically-sized slice of key-value pairs in an [`IndexMap`].
///
/// This supports indexed operations much like a `[(K, V)]` slice,
/// but not any hashed operations on the map keys.
///
/// Unlike `IndexMap`, `Slice` does consider the order for [`PartialEq`]
/// and [`Eq`], and it also implements [`PartialOrd`], [`Ord`], and [`Hash`].
#[repr(transparent)]
pub struct Slice<K, V> {
    pub(crate) entries: [Bucket<K, V>],
}

// SAFETY: `Slice<K, V>` is a transparent wrapper around `[Bucket<K, V>]`,
// and reference lifetimes are bound together in function signatures.
#[allow(unsafe_code)]
impl<K, V> Slice<K, V> {
    pub(super) const fn from_slice(entries: &[Bucket<K, V>]) -> &Self {
        unsafe { &*(entries as *const [Bucket<K, V>] as *const Self) }
    }

    pub(super) fn from_mut_slice(entries: &mut [Bucket<K, V>]) -> &mut Self {
        unsafe { &mut *(entries as *mut [Bucket<K, V>] as *mut Self) }
    }

    pub(super) fn from_boxed(entries: Box<[Bucket<K, V>]>) -> Box<Self> {
        unsafe { Box::from_raw(Box::into_raw(entries) as *mut Self) }
    }

    fn into_boxed(self: Box<Self>) -> Box<[Bucket<K, V>]> {
        unsafe { Box::from_raw(Box::into_raw(self) as *mut [Bucket<K, V>]) }
    }
}

impl<K, V> Slice<K, V> {
    pub(crate) fn into_entries(self: Box<Self>) -> Vec<Bucket<K, V>> {
        self.into_boxed().into_vec()
    }

    /// Returns an empty slice.
    pub const fn new<'a>() -> &'a Self {
        Self::from_slice(&[])
    }

    /// Returns an empty mutable slice.
    pub fn new_mut<'a>() -> &'a mut Self {
        Self::from_mut_slice(&mut [])
    }

    /// Return the number of key-value pairs in the map slice.
    #[inline]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the map slice contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a key-value pair by index.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.entries.get(index).map(Bucket::refs)
    }

    /// Get a key-value pair by index, with mutable access to the value.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.entries.get_mut(index).map(Bucket::ref_mut)
    }

    /// Returns a slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Self> {
        let range = try_simplify_range(range, self.entries.len())?;
        self.entries.get(range).map(Slice::from_slice)
    }

    /// Returns a mutable slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    pub fn get_range_mut<R: RangeBounds<usize>>(&mut self, range: R) -> Option<&mut Self> {
        let range = try_simplify_range(range, self.entries.len())?;
        self.entries.get_mut(range).map(Slice::from_mut_slice)
    }

    /// Get the first key-value pair.
    pub fn first(&self) -> Option<(&K, &V)> {
        self.entries.first().map(Bucket::refs)
    }

    /// Get the first key-value pair, with mutable access to the value.
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.entries.first_mut().map(Bucket::ref_mut)
    }

    /// Get the last key-value pair.
    pub fn last(&self) -> Option<(&K, &V)> {
        self.entries.last().map(Bucket::refs)
    }

    /// Get the last key-value pair, with mutable access to the value.
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.entries.last_mut().map(Bucket::ref_mut)
    }

    /// Divides one slice into two at an index.
    ///
    /// ***Panics*** if `index > len`.
    #[track_caller]
    pub fn split_at(&self, index: usize) -> (&Self, &Self) {
        let (first, second) = self.entries.split_at(index);
        (Self::from_slice(first), Self::from_slice(second))
    }

    /// Divides one mutable slice into two at an index.
    ///
    /// ***Panics*** if `index > len`.
    #[track_caller]
    pub fn split_at_mut(&mut self, index: usize) -> (&mut Self, &mut Self) {
        let (first, second) = self.entries.split_at_mut(index);
        (Self::from_mut_slice(first), Self::from_mut_slice(second))
    }

    /// Returns the first key-value pair and the rest of the slice,
    /// or `None` if it is empty.
    pub fn split_first(&self) -> Option<((&K, &V), &Self)> {
        if let [first, rest @ ..] = &self.entries {
            Some((first.refs(), Self::from_slice(rest)))
        } else {
            None
        }
    }

    /// Returns the first key-value pair and the rest of the slice,
    /// with mutable access to the value, or `None` if it is empty.
    pub fn split_first_mut(&mut self) -> Option<((&K, &mut V), &mut Self)> {
        if let [first, rest @ ..] = &mut self.entries {
            Some((first.ref_mut(), Self::from_mut_slice(rest)))
        } else {
            None
        }
    }

    /// Returns the last key-value pair and the rest of the slice,
    /// or `None` if it is empty.
    pub fn split_last(&self) -> Option<((&K, &V), &Self)> {
        if let [rest @ .., last] = &self.entries {
            Some((last.refs(), Self::from_slice(rest)))
        } else {
            None
        }
    }

    /// Returns the last key-value pair and the rest of the slice,
    /// with mutable access to the value, or `None` if it is empty.
    pub fn split_last_mut(&mut self) -> Option<((&K, &mut V), &mut Self)> {
        if let [rest @ .., last] = &mut self.entries {
            Some((last.ref_mut(), Self::from_mut_slice(rest)))
        } else {
            None
        }
    }

    /// Return an iterator over the key-value pairs of the map slice.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(&self.entries)
    }

    /// Return an iterator over the key-value pairs of the map slice.
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(&mut self.entries)
    }

    /// Return an iterator over the keys of the map slice.
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys::new(&self.entries)
    }

    /// Return an owning iterator over the keys of the map slice.
    pub fn into_keys(self: Box<Self>) -> IntoKeys<K, V> {
        IntoKeys::new(self.into_entries())
    }

    /// Return an iterator over the values of the map slice.
    pub fn values(&self) -> Values<'_, K, V> {
        Values::new(&self.entries)
    }

    /// Return an iterator over mutable references to the the values of the map slice.
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut::new(&mut self.entries)
    }

    /// Return an owning iterator over the values of the map slice.
    pub fn into_values(self: Box<Self>) -> IntoValues<K, V> {
        IntoValues::new(self.into_entries())
    }

    /// Search over a sorted map for a key.
    ///
    /// Returns the position where that key is present, or the position where it can be inserted to
    /// maintain the sort. See [`slice::binary_search`] for more details.
    ///
    /// Computes in **O(log(n))** time, which is notably less scalable than looking the key up in
    /// the map this is a slice from using [`IndexMap::get_index_of`], but this can also position
    /// missing keys.
    pub fn binary_search_keys(&self, x: &K) -> Result<usize, usize>
    where
        K: Ord,
    {
        self.binary_search_by(|p, _| p.cmp(x))
    }

    /// Search over a sorted map with a comparator function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> Ordering,
    {
        self.entries.binary_search_by(move |a| f(&a.key, &a.value))
    }

    /// Search over a sorted map with an extraction function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by_key`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> B,
        B: Ord,
    {
        self.binary_search_by(|k, v| f(k, v).cmp(b))
    }

    /// Checks if the keys of this slice are sorted.
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        K: PartialOrd,
    {
        self.entries.is_sorted_by(|a, b| a.key <= b.key)
    }

    /// Checks if this slice is sorted using the given comparator function.
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, mut cmp: F) -> bool
    where
        F: FnMut(&'a K, &'a V, &'a K, &'a V) -> bool,
    {
        self.entries
            .is_sorted_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value))
    }

    /// Checks if this slice is sorted using the given sort-key function.
    #[inline]
    pub fn is_sorted_by_key<'a, F, T>(&'a self, mut sort_key: F) -> bool
    where
        F: FnMut(&'a K, &'a V) -> T,
        T: PartialOrd,
    {
        self.entries
            .is_sorted_by_key(move |a| sort_key(&a.key, &a.value))
    }

    /// Returns the index of the partition point of a sorted map according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// See [`slice::partition_point`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[must_use]
    pub fn partition_point<P>(&self, mut pred: P) -> usize
    where
        P: FnMut(&K, &V) -> bool,
    {
        self.entries
            .partition_point(move |a| pred(&a.key, &a.value))
    }

    /// Get an array of `N` key-value pairs by `N` indices
    ///
    /// Valid indices are *0 <= index < self.len()* and each index needs to be unique.
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[(&K, &mut V); N], GetDisjointMutError> {
        let indices = indices.map(Some);
        let key_values = self.get_disjoint_opt_mut(indices)?;
        Ok(key_values.map(Option::unwrap))
    }

    #[allow(unsafe_code)]
    pub(crate) fn get_disjoint_opt_mut<const N: usize>(
        &mut self,
        indices: [Option<usize>; N],
    ) -> Result<[Option<(&K, &mut V)>; N], GetDisjointMutError> {
        // SAFETY: Can't allow duplicate indices as we would return several mutable refs to the same data.
        let len = self.len();
        for i in 0..N {
            if let Some(idx) = indices[i] {
                if idx >= len {
                    return Err(GetDisjointMutError::IndexOutOfBounds);
                } else if indices[..i].contains(&Some(idx)) {
                    return Err(GetDisjointMutError::OverlappingIndices);
                }
            }
        }

        let entries_ptr = self.entries.as_mut_ptr();
        let out = indices.map(|idx_opt| {
            match idx_opt {
                Some(idx) => {
                    // SAFETY: The base pointer is valid as it comes from a slice and the reference is always
                    // in-bounds & unique as we've already checked the indices above.
                    let kv = unsafe { (*(entries_ptr.add(idx))).ref_mut() };
                    Some(kv)
                }
                None => None,
            }
        });

        Ok(out)
    }
}

impl<'a, K, V> IntoIterator for &'a Slice<K, V> {
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut Slice<K, V> {
    type IntoIter = IterMut<'a, K, V>;
    type Item = (&'a K, &'a mut V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> IntoIterator for Box<Slice<K, V>> {
    type IntoIter = IntoIter<K, V>;
    type Item = (K, V);

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.into_entries())
    }
}

impl<K, V> Default for &'_ Slice<K, V> {
    fn default() -> Self {
        Slice::from_slice(&[])
    }
}

impl<K, V> Default for &'_ mut Slice<K, V> {
    fn default() -> Self {
        Slice::from_mut_slice(&mut [])
    }
}

impl<K, V> Default for Box<Slice<K, V>> {
    fn default() -> Self {
        Slice::from_boxed(Box::default())
    }
}

impl<K: Clone, V: Clone> Clone for Box<Slice<K, V>> {
    fn clone(&self) -> Self {
        Slice::from_boxed(self.entries.to_vec().into_boxed_slice())
    }
}

impl<K: Copy, V: Copy> From<&Slice<K, V>> for Box<Slice<K, V>> {
    fn from(slice: &Slice<K, V>) -> Self {
        Slice::from_boxed(Box::from(&slice.entries))
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Slice<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<K, V, K2, V2> PartialEq<Slice<K2, V2>> for Slice<K, V>
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &Slice<K2, V2>) -> bool {
        slice_eq(&self.entries, &other.entries, |b1, b2| {
            b1.key == b2.key && b1.value == b2.value
        })
    }
}

impl<K, V, K2, V2> PartialEq<[(K2, V2)]> for Slice<K, V>
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &[(K2, V2)]) -> bool {
        slice_eq(&self.entries, other, |b, t| b.key == t.0 && b.value == t.1)
    }
}

impl<K, V, K2, V2> PartialEq<Slice<K2, V2>> for [(K, V)]
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &Slice<K2, V2>) -> bool {
        slice_eq(self, &other.entries, |t, b| t.0 == b.key && t.1 == b.value)
    }
}

impl<K, V, K2, V2, const N: usize> PartialEq<[(K2, V2); N]> for Slice<K, V>
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &[(K2, V2); N]) -> bool {
        <Self as PartialEq<[_]>>::eq(self, other)
    }
}

impl<K, V, const N: usize, K2, V2> PartialEq<Slice<K2, V2>> for [(K, V); N]
where
    K: PartialEq<K2>,
    V: PartialEq<V2>,
{
    fn eq(&self, other: &Slice<K2, V2>) -> bool {
        <[_] as PartialEq<_>>::eq(self, other)
    }
}

impl<K: Eq, V: Eq> Eq for Slice<K, V> {}

impl<K: PartialOrd, V: PartialOrd> PartialOrd for Slice<K, V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other)
    }
}

impl<K: Ord, V: Ord> Ord for Slice<K, V> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other)
    }
}

impl<K: Hash, V: Hash> Hash for Slice<K, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for (key, value) in self {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl<K, V> Index<usize> for Slice<K, V> {
    type Output = V;

    fn index(&self, index: usize) -> &V {
        &self.entries[index].value
    }
}

impl<K, V> IndexMut<usize> for Slice<K, V> {
    fn index_mut(&mut self, index: usize) -> &mut V {
        &mut self.entries[index].value
    }
}

// We can't have `impl<I: RangeBounds<usize>> Index<I>` because that conflicts
// both upstream with `Index<usize>` and downstream with `Index<&Q>`.
// Instead, we repeat the implementations for all the core range types.
macro_rules! impl_index {
    ($($range:ty),*) => {$(
        impl<K, V, S> Index<$range> for IndexMap<K, V, S> {
            type Output = Slice<K, V>;

            fn index(&self, range: $range) -> &Self::Output {
                Slice::from_slice(&self.as_entries()[range])
            }
        }

        impl<K, V, S> IndexMut<$range> for IndexMap<K, V, S> {
            fn index_mut(&mut self, range: $range) -> &mut Self::Output {
                Slice::from_mut_slice(&mut self.as_entries_mut()[range])
            }
        }

        impl<K, V> Index<$range> for Slice<K, V> {
            type Output = Slice<K, V>;

            fn index(&self, range: $range) -> &Self {
                Self::from_slice(&self.entries[range])
            }
        }

        impl<K, V> IndexMut<$range> for Slice<K, V> {
            fn index_mut(&mut self, range: $range) -> &mut Self {
                Self::from_mut_slice(&mut self.entries[range])
            }
        }
    )*}
}
impl_index!(
    ops::Range<usize>,
    ops::RangeFrom<usize>,
    ops::RangeFull,
    ops::RangeInclusive<usize>,
    ops::RangeTo<usize>,
    ops::RangeToInclusive<usize>,
    (Bound<usize>, Bound<usize>)
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_index() {
        fn check(
            vec_slice: &[(i32, i32)],
            map_slice: &Slice<i32, i32>,
            sub_slice: &Slice<i32, i32>,
        ) {
            assert_eq!(map_slice as *const _, sub_slice as *const _);
            itertools::assert_equal(
                vec_slice.iter().copied(),
                map_slice.iter().map(|(&k, &v)| (k, v)),
            );
            itertools::assert_equal(vec_slice.iter().map(|(k, _)| k), map_slice.keys());
            itertools::assert_equal(vec_slice.iter().map(|(_, v)| v), map_slice.values());
        }

        let vec: Vec<(i32, i32)> = (0..10).map(|i| (i, i * i)).collect();
        let map: IndexMap<i32, i32> = vec.iter().cloned().collect();
        let slice = map.as_slice();

        // RangeFull
        check(&vec[..], &map[..], &slice[..]);

        for i in 0usize..10 {
            // Index
            assert_eq!(vec[i].1, map[i]);
            assert_eq!(vec[i].1, slice[i]);
            assert_eq!(map[&(i as i32)], map[i]);
            assert_eq!(map[&(i as i32)], slice[i]);

            // RangeFrom
            check(&vec[i..], &map[i..], &slice[i..]);

            // RangeTo
            check(&vec[..i], &map[..i], &slice[..i]);

            // RangeToInclusive
            check(&vec[..=i], &map[..=i], &slice[..=i]);

            // (Bound<usize>, Bound<usize>)
            let bounds = (Bound::Excluded(i), Bound::Unbounded);
            check(&vec[i + 1..], &map[bounds], &slice[bounds]);

            for j in i..=10 {
                // Range
                check(&vec[i..j], &map[i..j], &slice[i..j]);
            }

            for j in i..10 {
                // RangeInclusive
                check(&vec[i..=j], &map[i..=j], &slice[i..=j]);
            }
        }
    }

    #[test]
    fn slice_index_mut() {
        fn check_mut(
            vec_slice: &[(i32, i32)],
            map_slice: &mut Slice<i32, i32>,
            sub_slice: &mut Slice<i32, i32>,
        ) {
            assert_eq!(map_slice, sub_slice);
            itertools::assert_equal(
                vec_slice.iter().copied(),
                map_slice.iter_mut().map(|(&k, &mut v)| (k, v)),
            );
            itertools::assert_equal(
                vec_slice.iter().map(|&(_, v)| v),
                map_slice.values_mut().map(|&mut v| v),
            );
        }

        let vec: Vec<(i32, i32)> = (0..10).map(|i| (i, i * i)).collect();
        let mut map: IndexMap<i32, i32> = vec.iter().cloned().collect();
        let mut map2 = map.clone();
        let slice = map2.as_mut_slice();

        // RangeFull
        check_mut(&vec[..], &mut map[..], &mut slice[..]);

        for i in 0usize..10 {
            // IndexMut
            assert_eq!(&mut map[i], &mut slice[i]);

            // RangeFrom
            check_mut(&vec[i..], &mut map[i..], &mut slice[i..]);

            // RangeTo
            check_mut(&vec[..i], &mut map[..i], &mut slice[..i]);

            // RangeToInclusive
            check_mut(&vec[..=i], &mut map[..=i], &mut slice[..=i]);

            // (Bound<usize>, Bound<usize>)
            let bounds = (Bound::Excluded(i), Bound::Unbounded);
            check_mut(&vec[i + 1..], &mut map[bounds], &mut slice[bounds]);

            for j in i..=10 {
                // Range
                check_mut(&vec[i..j], &mut map[i..j], &mut slice[i..j]);
            }

            for j in i..10 {
                // RangeInclusive
                check_mut(&vec[i..=j], &mut map[i..=j], &mut slice[i..=j]);
            }
        }
    }

    #[test]
    fn slice_new() {
        let slice: &Slice<i32, i32> = Slice::new();
        assert!(slice.is_empty());
        assert_eq!(slice.len(), 0);
    }

    #[test]
    fn slice_new_mut() {
        let slice: &mut Slice<i32, i32> = Slice::new_mut();
        assert!(slice.is_empty());
        assert_eq!(slice.len(), 0);
    }

    #[test]
    fn slice_get_index_mut() {
        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();

        {
            let (key, value) = slice.get_index_mut(0).unwrap();
            assert_eq!(*key, 0);
            assert_eq!(*value, 0);

            *value = 11;
        }

        assert_eq!(slice[0], 11);

        {
            let result = slice.get_index_mut(11);
            assert!(result.is_none());
        }
    }

    #[test]
    fn slice_split_first() {
        let slice: &mut Slice<i32, i32> = Slice::new_mut();
        let result = slice.split_first();
        assert!(result.is_none());

        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();

        {
            let (first, rest) = slice.split_first().unwrap();
            assert_eq!(first, (&0, &0));
            assert_eq!(rest.len(), 9);
        }
        assert_eq!(slice.len(), 10);
    }

    #[test]
    fn slice_split_first_mut() {
        let slice: &mut Slice<i32, i32> = Slice::new_mut();
        let result = slice.split_first_mut();
        assert!(result.is_none());

        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();

        {
            let (first, rest) = slice.split_first_mut().unwrap();
            assert_eq!(first, (&0, &mut 0));
            assert_eq!(rest.len(), 9);

            *first.1 = 11;
        }
        assert_eq!(slice.len(), 10);
        assert_eq!(slice[0], 11);
    }

    #[test]
    fn slice_split_last() {
        let slice: &mut Slice<i32, i32> = Slice::new_mut();
        let result = slice.split_last();
        assert!(result.is_none());

        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();

        {
            let (last, rest) = slice.split_last().unwrap();
            assert_eq!(last, (&9, &81));
            assert_eq!(rest.len(), 9);
        }
        assert_eq!(slice.len(), 10);
    }

    #[test]
    fn slice_split_last_mut() {
        let slice: &mut Slice<i32, i32> = Slice::new_mut();
        let result = slice.split_last_mut();
        assert!(result.is_none());

        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();

        {
            let (last, rest) = slice.split_last_mut().unwrap();
            assert_eq!(last, (&9, &mut 81));
            assert_eq!(rest.len(), 9);

            *last.1 = 100;
        }

        assert_eq!(slice.len(), 10);
        assert_eq!(slice[slice.len() - 1], 100);
    }

    #[test]
    fn slice_get_range() {
        let mut map: IndexMap<i32, i32> = (0..10).map(|i| (i, i * i)).collect();
        let slice: &mut Slice<i32, i32> = map.as_mut_slice();
        let subslice = slice.get_range(3..6).unwrap();
        assert_eq!(subslice.len(), 3);
        assert_eq!(subslice, &[(3, 9), (4, 16), (5, 25)]);
    }
}
