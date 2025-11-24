// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ule::*;
use crate::varzerovec::owned::VarZeroVecOwned;
use crate::varzerovec::vec::VarZeroVecInner;
use crate::vecs::VarZeroVecFormat;
use crate::{VarZeroSlice, VarZeroVec};
use crate::{ZeroSlice, ZeroVec};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::mem;
use core::ops::Range;

/// Trait abstracting over [`ZeroVec`] and [`VarZeroVec`], for use in [`ZeroMap`](super::ZeroMap). **You
/// should not be implementing or calling this trait directly.**
///
/// The T type is the type received by [`Self::zvl_binary_search()`], as well as the one used
/// for human-readable serialization.
///
/// Methods are prefixed with `zvl_*` to avoid clashes with methods on the types themselves
pub trait ZeroVecLike<T: ?Sized> {
    /// The type returned by `Self::get()`
    type GetType: ?Sized + 'static;
    /// A fully borrowed version of this
    type SliceVariant: ZeroVecLike<T, GetType = Self::GetType> + ?Sized;

    /// Create a new, empty borrowed variant
    fn zvl_new_borrowed() -> &'static Self::SliceVariant;

    /// Search for a key in a sorted vector, returns `Ok(index)` if found,
    /// returns `Err(insert_index)` if not found, where `insert_index` is the
    /// index where it should be inserted to maintain sort order.
    fn zvl_binary_search(&self, k: &T) -> Result<usize, usize>
    where
        T: Ord;
    /// Search for a key within a certain range in a sorted vector.
    /// Returns `None` if the range is out of bounds, and
    /// `Ok` or `Err` in the same way as `zvl_binary_search`.
    /// Indices are returned relative to the start of the range.
    fn zvl_binary_search_in_range(
        &self,
        k: &T,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>>
    where
        T: Ord;

    /// Search for a key in a sorted vector by a predicate, returns `Ok(index)` if found,
    /// returns `Err(insert_index)` if not found, where `insert_index` is the
    /// index where it should be inserted to maintain sort order.
    fn zvl_binary_search_by(&self, predicate: impl FnMut(&T) -> Ordering) -> Result<usize, usize>;
    /// Search for a key within a certain range in a sorted vector by a predicate.
    /// Returns `None` if the range is out of bounds, and
    /// `Ok` or `Err` in the same way as `zvl_binary_search`.
    /// Indices are returned relative to the start of the range.
    fn zvl_binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>>;

    /// Get element at `index`
    fn zvl_get(&self, index: usize) -> Option<&Self::GetType>;
    /// The length of this vector
    fn zvl_len(&self) -> usize;
    /// Check if this vector is in ascending order according to `T`s `Ord` impl
    fn zvl_is_ascending(&self) -> bool
    where
        T: Ord,
    {
        if let Some(first) = self.zvl_get(0) {
            let mut prev = first;
            for i in 1..self.zvl_len() {
                #[expect(clippy::unwrap_used)] // looping over the valid indices
                let curr = self.zvl_get(i).unwrap();
                if Self::get_cmp_get(prev, curr) != Ordering::Less {
                    return false;
                }
                prev = curr;
            }
        }
        true
    }
    /// Check if this vector is empty
    fn zvl_is_empty(&self) -> bool {
        self.zvl_len() == 0
    }

    /// Construct a borrowed variant by borrowing from `&self`.
    ///
    /// This function behaves like `&'b self -> Self::SliceVariant<'b>`,
    /// where `'b` is the lifetime of the reference to this object.
    ///
    /// Note: We rely on the compiler recognizing `'a` and `'b` as covariant and
    /// casting `&'b Self<'a>` to `&'b Self<'b>` when this gets called, which works
    /// out for `ZeroVec` and `VarZeroVec` containers just fine.
    fn zvl_as_borrowed(&self) -> &Self::SliceVariant;

    /// Compare this type with a `Self::GetType`. This must produce the same result as
    /// if `g` were converted to `Self`
    #[inline]
    fn t_cmp_get(t: &T, g: &Self::GetType) -> Ordering
    where
        T: Ord,
    {
        Self::zvl_get_as_t(g, |g| t.cmp(g))
    }

    /// Compare two values of `Self::GetType`. This must produce the same result as
    /// if both `a` and `b` were converted to `Self`
    #[inline]
    fn get_cmp_get(a: &Self::GetType, b: &Self::GetType) -> Ordering
    where
        T: Ord,
    {
        Self::zvl_get_as_t(a, |a| Self::zvl_get_as_t(b, |b| a.cmp(b)))
    }

    /// Obtain a reference to T, passed to a closure
    ///
    /// This uses a callback because it's not possible to return owned-or-borrowed
    /// types without GATs
    ///
    /// Impls should guarantee that the callback function is be called exactly once.
    fn zvl_get_as_t<R>(g: &Self::GetType, f: impl FnOnce(&T) -> R) -> R;
}

/// Trait abstracting over [`ZeroVec`] and [`VarZeroVec`], for use in [`ZeroMap`](super::ZeroMap). **You
/// should not be implementing or calling this trait directly.**
///
/// This trait augments [`ZeroVecLike`] with methods allowing for mutation of the underlying
/// vector for owned vector types.
///
/// Methods are prefixed with `zvl_*` to avoid clashes with methods on the types themselves
pub trait MutableZeroVecLike<'a, T: ?Sized>: ZeroVecLike<T> {
    /// The type returned by `Self::remove()` and `Self::replace()`
    type OwnedType;

    /// Insert an element at `index`
    fn zvl_insert(&mut self, index: usize, value: &T);
    /// Remove the element at `index` (panicking if nonexistant)
    fn zvl_remove(&mut self, index: usize) -> Self::OwnedType;
    /// Replace the element at `index` with another one, returning the old element
    fn zvl_replace(&mut self, index: usize, value: &T) -> Self::OwnedType;
    /// Push an element to the end of this vector
    fn zvl_push(&mut self, value: &T);
    /// Create a new, empty vector, with given capacity
    fn zvl_with_capacity(cap: usize) -> Self;
    /// Remove all elements from the vector
    fn zvl_clear(&mut self);
    /// Reserve space for `addl` additional elements
    fn zvl_reserve(&mut self, addl: usize);
    /// Applies the permutation such that `before.zvl_get(permutation[i]) == after.zvl_get(i)`.
    ///
    /// # Panics
    /// If `permutation` is not a valid permutation of length `zvl_len()`.
    fn zvl_permute(&mut self, permutation: &mut [usize]);

    /// Convert an owned value to a borrowed T
    fn owned_as_t(o: &Self::OwnedType) -> &T;

    /// Construct from the borrowed version of the type
    ///
    /// These are useful to ensure serialization parity between borrowed and owned versions
    fn zvl_from_borrowed(b: &'a Self::SliceVariant) -> Self;
    /// Extract the inner borrowed variant if possible. Returns `None` if the data is owned.
    ///
    /// This function behaves like `&'_ self -> Self::SliceVariant<'a>`,
    /// where `'a` is the lifetime of this object's borrowed data.
    ///
    /// This function is similar to matching the `Borrowed` variant of `ZeroVec`
    /// or `VarZeroVec`, returning the inner borrowed type.
    fn zvl_as_borrowed_inner(&self) -> Option<&'a Self::SliceVariant>;
}

impl<'a, T> ZeroVecLike<T> for ZeroVec<'a, T>
where
    T: 'a + AsULE + Copy,
{
    type GetType = T::ULE;
    type SliceVariant = ZeroSlice<T>;

    fn zvl_new_borrowed() -> &'static Self::SliceVariant {
        ZeroSlice::<T>::new_empty()
    }
    fn zvl_binary_search(&self, k: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        ZeroSlice::binary_search(self, k)
    }
    fn zvl_binary_search_in_range(&self, k: &T, range: Range<usize>) -> Option<Result<usize, usize>>
    where
        T: Ord,
    {
        let zs: &ZeroSlice<T> = self;
        zs.zvl_binary_search_in_range(k, range)
    }
    fn zvl_binary_search_by(
        &self,
        mut predicate: impl FnMut(&T) -> Ordering,
    ) -> Result<usize, usize> {
        ZeroSlice::binary_search_by(self, |probe| predicate(&probe))
    }
    fn zvl_binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        let zs: &ZeroSlice<T> = self;
        zs.zvl_binary_search_in_range_by(predicate, range)
    }
    fn zvl_get(&self, index: usize) -> Option<&T::ULE> {
        self.get_ule_ref(index)
    }
    fn zvl_len(&self) -> usize {
        ZeroSlice::len(self)
    }
    fn zvl_as_borrowed(&self) -> &ZeroSlice<T> {
        self
    }
    #[inline]
    fn zvl_get_as_t<R>(g: &Self::GetType, f: impl FnOnce(&T) -> R) -> R {
        f(&T::from_unaligned(*g))
    }
}

impl<T> ZeroVecLike<T> for ZeroSlice<T>
where
    T: AsULE + Copy,
{
    type GetType = T::ULE;
    type SliceVariant = ZeroSlice<T>;

    fn zvl_new_borrowed() -> &'static Self::SliceVariant {
        ZeroSlice::<T>::new_empty()
    }
    fn zvl_binary_search(&self, k: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        ZeroSlice::binary_search(self, k)
    }
    fn zvl_binary_search_in_range(&self, k: &T, range: Range<usize>) -> Option<Result<usize, usize>>
    where
        T: Ord,
    {
        let subslice = self.get_subslice(range)?;
        Some(ZeroSlice::binary_search(subslice, k))
    }
    fn zvl_binary_search_by(
        &self,
        mut predicate: impl FnMut(&T) -> Ordering,
    ) -> Result<usize, usize> {
        ZeroSlice::binary_search_by(self, |probe| predicate(&probe))
    }
    fn zvl_binary_search_in_range_by(
        &self,
        mut predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        let subslice = self.get_subslice(range)?;
        Some(ZeroSlice::binary_search_by(subslice, |probe| {
            predicate(&probe)
        }))
    }
    fn zvl_get(&self, index: usize) -> Option<&T::ULE> {
        self.get_ule_ref(index)
    }
    fn zvl_len(&self) -> usize {
        ZeroSlice::len(self)
    }
    fn zvl_as_borrowed(&self) -> &ZeroSlice<T> {
        self
    }

    #[inline]
    fn zvl_get_as_t<R>(g: &Self::GetType, f: impl FnOnce(&T) -> R) -> R {
        f(&T::from_unaligned(*g))
    }
}

impl<'a, T> MutableZeroVecLike<'a, T> for ZeroVec<'a, T>
where
    T: AsULE + Copy + 'static,
{
    type OwnedType = T;
    fn zvl_insert(&mut self, index: usize, value: &T) {
        self.with_mut(|v| v.insert(index, value.to_unaligned()))
    }
    fn zvl_remove(&mut self, index: usize) -> T {
        T::from_unaligned(self.with_mut(|v| v.remove(index)))
    }
    fn zvl_replace(&mut self, index: usize, value: &T) -> T {
        #[expect(clippy::indexing_slicing)]
        let unaligned = self.with_mut(|vec| {
            debug_assert!(index < vec.len());
            mem::replace(&mut vec[index], value.to_unaligned())
        });
        T::from_unaligned(unaligned)
    }
    fn zvl_push(&mut self, value: &T) {
        self.with_mut(|v| v.push(value.to_unaligned()))
    }
    fn zvl_with_capacity(cap: usize) -> Self {
        if cap == 0 {
            ZeroVec::new()
        } else {
            ZeroVec::new_owned(Vec::with_capacity(cap))
        }
    }
    fn zvl_clear(&mut self) {
        self.with_mut(|v| v.clear())
    }
    fn zvl_reserve(&mut self, addl: usize) {
        self.with_mut(|v| v.reserve(addl))
    }

    fn owned_as_t(o: &Self::OwnedType) -> &T {
        o
    }

    fn zvl_from_borrowed(b: &'a ZeroSlice<T>) -> Self {
        b.as_zerovec()
    }
    fn zvl_as_borrowed_inner(&self) -> Option<&'a ZeroSlice<T>> {
        self.as_maybe_borrowed()
    }

    #[expect(clippy::indexing_slicing)] // documented panic
    fn zvl_permute(&mut self, permutation: &mut [usize]) {
        assert_eq!(permutation.len(), self.zvl_len());

        let vec = self.to_mut_slice();

        for cycle_start in 0..permutation.len() {
            let mut curr = cycle_start;
            let mut next = permutation[curr];

            while next != cycle_start {
                vec.swap(curr, next);
                // Make curr a self-cycle so we don't use it as a cycle_start later
                permutation[curr] = curr;
                curr = next;
                next = permutation[next];
            }
            permutation[curr] = curr;
        }
    }
}

impl<'a, T, F> ZeroVecLike<T> for VarZeroVec<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    F: VarZeroVecFormat,
{
    type GetType = T;
    type SliceVariant = VarZeroSlice<T, F>;

    fn zvl_new_borrowed() -> &'static Self::SliceVariant {
        VarZeroSlice::<T, F>::new_empty()
    }
    fn zvl_binary_search(&self, k: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search(k)
    }
    fn zvl_binary_search_in_range(&self, k: &T, range: Range<usize>) -> Option<Result<usize, usize>>
    where
        T: Ord,
    {
        self.binary_search_in_range(k, range)
    }
    fn zvl_binary_search_by(&self, predicate: impl FnMut(&T) -> Ordering) -> Result<usize, usize> {
        self.binary_search_by(predicate)
    }
    fn zvl_binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        self.binary_search_in_range_by(predicate, range)
    }
    fn zvl_get(&self, index: usize) -> Option<&T> {
        self.get(index)
    }
    fn zvl_len(&self) -> usize {
        self.len()
    }

    fn zvl_as_borrowed(&self) -> &VarZeroSlice<T, F> {
        self.as_slice()
    }

    #[inline]
    fn zvl_get_as_t<R>(g: &Self::GetType, f: impl FnOnce(&T) -> R) -> R {
        f(g)
    }
}

impl<T, F> ZeroVecLike<T> for VarZeroSlice<T, F>
where
    T: VarULE,
    T: ?Sized,
    F: VarZeroVecFormat,
{
    type GetType = T;
    type SliceVariant = VarZeroSlice<T, F>;

    fn zvl_new_borrowed() -> &'static Self::SliceVariant {
        VarZeroSlice::<T, F>::new_empty()
    }
    fn zvl_binary_search(&self, k: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search(k)
    }
    fn zvl_binary_search_in_range(&self, k: &T, range: Range<usize>) -> Option<Result<usize, usize>>
    where
        T: Ord,
    {
        self.binary_search_in_range(k, range)
    }
    fn zvl_binary_search_by(&self, predicate: impl FnMut(&T) -> Ordering) -> Result<usize, usize> {
        self.binary_search_by(predicate)
    }
    fn zvl_binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        self.binary_search_in_range_by(predicate, range)
    }
    fn zvl_get(&self, index: usize) -> Option<&T> {
        self.get(index)
    }
    fn zvl_len(&self) -> usize {
        self.len()
    }

    fn zvl_as_borrowed(&self) -> &VarZeroSlice<T, F> {
        self
    }

    #[inline]
    fn zvl_get_as_t<R>(g: &Self::GetType, f: impl FnOnce(&T) -> R) -> R {
        f(g)
    }
}

impl<'a, T, F> MutableZeroVecLike<'a, T> for VarZeroVec<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    F: VarZeroVecFormat,
{
    type OwnedType = Box<T>;
    fn zvl_insert(&mut self, index: usize, value: &T) {
        self.make_mut().insert(index, value)
    }
    fn zvl_remove(&mut self, index: usize) -> Box<T> {
        let vec = self.make_mut();
        debug_assert!(index < vec.len());
        #[expect(clippy::unwrap_used)]
        let old = vec.get(index).unwrap().to_boxed();
        vec.remove(index);
        old
    }
    fn zvl_replace(&mut self, index: usize, value: &T) -> Box<T> {
        let vec = self.make_mut();
        debug_assert!(index < vec.len());
        #[expect(clippy::unwrap_used)]
        let old = vec.get(index).unwrap().to_boxed();
        vec.replace(index, value);
        old
    }
    fn zvl_push(&mut self, value: &T) {
        let len = self.len();
        self.make_mut().insert(len, value)
    }
    fn zvl_with_capacity(cap: usize) -> Self {
        if cap == 0 {
            VarZeroVec::new()
        } else {
            Self::from(VarZeroVecOwned::with_capacity(cap))
        }
    }
    fn zvl_clear(&mut self) {
        self.make_mut().clear()
    }
    fn zvl_reserve(&mut self, addl: usize) {
        self.make_mut().reserve(addl)
    }

    fn owned_as_t(o: &Self::OwnedType) -> &T {
        o
    }

    fn zvl_from_borrowed(b: &'a VarZeroSlice<T, F>) -> Self {
        b.as_varzerovec()
    }
    fn zvl_as_borrowed_inner(&self) -> Option<&'a VarZeroSlice<T, F>> {
        if let Self(VarZeroVecInner::Borrowed(b)) = *self {
            Some(b)
        } else {
            None
        }
    }

    #[expect(clippy::unwrap_used)] // documented panic
    fn zvl_permute(&mut self, permutation: &mut [usize]) {
        assert_eq!(permutation.len(), self.zvl_len());

        let mut result = VarZeroVecOwned::new();
        for &i in permutation.iter() {
            result.push(self.get(i).unwrap());
        }
        *self = Self(VarZeroVecInner::Owned(result));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_zerovec_binary_search_in_range() {
        let zv: ZeroVec<u16> = ZeroVec::from_slice_or_alloc(&[11, 22, 33, 44, 55, 66, 77]);

        // Full range search
        assert_eq!(zv.zvl_binary_search_in_range(&11, 0..7), Some(Ok(0)));
        assert_eq!(zv.zvl_binary_search_in_range(&12, 0..7), Some(Err(1)));
        assert_eq!(zv.zvl_binary_search_in_range(&44, 0..7), Some(Ok(3)));
        assert_eq!(zv.zvl_binary_search_in_range(&45, 0..7), Some(Err(4)));
        assert_eq!(zv.zvl_binary_search_in_range(&77, 0..7), Some(Ok(6)));
        assert_eq!(zv.zvl_binary_search_in_range(&78, 0..7), Some(Err(7)));

        // Out-of-range search
        assert_eq!(zv.zvl_binary_search_in_range(&44, 0..2), Some(Err(2)));
        assert_eq!(zv.zvl_binary_search_in_range(&44, 5..7), Some(Err(0)));

        // Offset search
        assert_eq!(zv.zvl_binary_search_in_range(&44, 2..5), Some(Ok(1)));
        assert_eq!(zv.zvl_binary_search_in_range(&45, 2..5), Some(Err(2)));

        // Out-of-bounds
        assert_eq!(zv.zvl_binary_search_in_range(&44, 0..100), None);
        assert_eq!(zv.zvl_binary_search_in_range(&44, 100..200), None);
    }

    #[test]
    fn test_permute() {
        let mut zv: ZeroVec<u16> = ZeroVec::from_slice_or_alloc(&[11, 22, 33, 44, 55, 66, 77]);
        let mut permutation = vec![3, 2, 1, 0, 6, 5, 4];
        zv.zvl_permute(&mut permutation);
        assert_eq!(&zv, &[44, 33, 22, 11, 77, 66, 55]);

        let mut vzv: VarZeroVec<str> = VarZeroVec::from(
            VarZeroVecOwned::try_from_elements(&["11", "22", "33", "44", "55", "66", "77"])
                .unwrap(),
        );
        let mut permutation = vec![3, 2, 1, 0, 6, 5, 4];
        vzv.zvl_permute(&mut permutation);
        assert_eq!(&vzv, &["44", "33", "22", "11", "77", "66", "55"]);
    }
}
