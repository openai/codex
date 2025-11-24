// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::VarZeroVecFormatError;
use crate::ule::*;
use core::cmp::Ordering;
use core::convert::TryFrom;
use core::marker::PhantomData;
use core::mem;
use core::ops::Range;

/// This trait allows switching between different possible internal
/// representations of VarZeroVec.
///
/// Currently this crate supports three formats: [`Index8`], [`Index16`] and [`Index32`],
/// with [`Index16`] being the default for all [`VarZeroVec`](super::VarZeroVec)
/// types unless explicitly specified otherwise.
///
/// Do not implement this trait, its internals may be changed in the future,
/// and all of its associated items are hidden from the docs.
pub trait VarZeroVecFormat: 'static + Sized {
    /// The type to use for the indexing array
    ///
    /// Safety: must be a ULE for which all byte sequences are allowed
    #[doc(hidden)]
    type Index: IntegerULE;
    /// The type to use for the length segment
    ///
    /// Safety: must be a ULE for which all byte sequences are allowed
    #[doc(hidden)]
    type Len: IntegerULE;
}

/// This trait represents various ULE types that can be used to represent an integer
///
/// Do not implement this trait, its internals may be changed in the future,
/// and all of its associated items are hidden from the docs.
#[doc(hidden)]
pub unsafe trait IntegerULE: ULE {
    /// The error to show when unable to construct a vec
    #[doc(hidden)]
    const TOO_LARGE_ERROR: &'static str;

    /// Safety: must be sizeof(self)
    #[doc(hidden)]
    const SIZE: usize;

    /// Safety: must be maximum integral value represented here
    #[doc(hidden)]
    const MAX_VALUE: u32;

    /// Safety: Must roundtrip with from_usize and represent the correct
    /// integral value
    #[doc(hidden)]
    fn iule_to_usize(self) -> usize;

    #[doc(hidden)]
    fn iule_from_usize(x: usize) -> Option<Self>;

    /// Safety: Should always convert a buffer into an array of Self with the correct length
    #[doc(hidden)]
    #[cfg(feature = "alloc")]
    fn iule_from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut [Self];
}

/// This is a [`VarZeroVecFormat`] that stores u8s in the index array, and a u8 for a length.
///
/// Will have a smaller data size, but it's *extremely* likely for larger arrays
/// to be unrepresentable (and error on construction). Should probably be used
/// for known-small arrays, where all but the last field are known-small.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::exhaustive_structs)] // marker
pub struct Index8;

/// This is a [`VarZeroVecFormat`] that stores u16s in the index array, and a u16 for a length.
///
/// Will have a smaller data size, but it's more likely for larger arrays
/// to be unrepresentable (and error on construction)
///
/// This is the default index size used by all [`VarZeroVec`](super::VarZeroVec) types.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::exhaustive_structs)] // marker
pub struct Index16;

/// This is a [`VarZeroVecFormat`] that stores u32s in the index array, and a u32 for a length.
/// Will have a larger data size, but will support large arrays without
/// problems.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::exhaustive_structs)] // marker
pub struct Index32;

impl VarZeroVecFormat for Index8 {
    type Index = u8;
    type Len = u8;
}

impl VarZeroVecFormat for Index16 {
    type Index = RawBytesULE<2>;
    type Len = RawBytesULE<2>;
}

impl VarZeroVecFormat for Index32 {
    type Index = RawBytesULE<4>;
    type Len = RawBytesULE<4>;
}

unsafe impl IntegerULE for u8 {
    const TOO_LARGE_ERROR: &'static str = "Attempted to build VarZeroVec out of elements that \
                                     cumulatively are larger than a u8 in size";
    const SIZE: usize = mem::size_of::<Self>();
    const MAX_VALUE: u32 = u8::MAX as u32;
    #[inline]
    fn iule_to_usize(self) -> usize {
        self as usize
    }
    #[inline]
    fn iule_from_usize(u: usize) -> Option<Self> {
        u8::try_from(u).ok()
    }
    #[inline]
    #[cfg(feature = "alloc")]
    fn iule_from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut [Self] {
        bytes
    }
}

unsafe impl IntegerULE for RawBytesULE<2> {
    const TOO_LARGE_ERROR: &'static str = "Attempted to build VarZeroVec out of elements that \
                                     cumulatively are larger than a u16 in size";
    const SIZE: usize = mem::size_of::<Self>();
    const MAX_VALUE: u32 = u16::MAX as u32;
    #[inline]
    fn iule_to_usize(self) -> usize {
        self.as_unsigned_int() as usize
    }
    #[inline]
    fn iule_from_usize(u: usize) -> Option<Self> {
        u16::try_from(u).ok().map(u16::to_unaligned)
    }
    #[inline]
    #[cfg(feature = "alloc")]
    fn iule_from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut [Self] {
        Self::from_bytes_unchecked_mut(bytes)
    }
}

unsafe impl IntegerULE for RawBytesULE<4> {
    const TOO_LARGE_ERROR: &'static str = "Attempted to build VarZeroVec out of elements that \
                                     cumulatively are larger than a u32 in size";
    const SIZE: usize = mem::size_of::<Self>();
    const MAX_VALUE: u32 = u32::MAX;
    #[inline]
    fn iule_to_usize(self) -> usize {
        self.as_unsigned_int() as usize
    }
    #[inline]
    fn iule_from_usize(u: usize) -> Option<Self> {
        u32::try_from(u).ok().map(u32::to_unaligned)
    }
    #[inline]
    #[cfg(feature = "alloc")]
    fn iule_from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut [Self] {
        Self::from_bytes_unchecked_mut(bytes)
    }
}

/// A more parsed version of `VarZeroSlice`. This type is where most of the VarZeroVec
/// internal representation code lies.
///
/// This is *basically* an `&'a [u8]` to a zero copy buffer, but split out into
/// the buffer components. Logically this is capable of behaving as
/// a `&'a [T::VarULE]`, but since `T::VarULE` is unsized that type does not actually
/// exist.
///
/// See [`VarZeroVecComponents::parse_bytes()`] for information on the internal invariants involved
#[derive(Debug)]
pub struct VarZeroVecComponents<'a, T: ?Sized, F> {
    /// The number of elements
    len: u32,
    /// The list of indices into the `things` slice
    /// Since the first element is always at things[0], the first element of the indices array is for the *second* element
    indices: &'a [u8],
    /// The contiguous list of `T::VarULE`s
    things: &'a [u8],
    marker: PhantomData<(&'a T, F)>,
}

// #[derive()] won't work here since we do not want it to be
// bound on T: Copy
impl<'a, T: ?Sized, F> Copy for VarZeroVecComponents<'a, T, F> {}
impl<'a, T: ?Sized, F> Clone for VarZeroVecComponents<'a, T, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: VarULE + ?Sized, F> Default for VarZeroVecComponents<'a, T, F> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T: VarULE + ?Sized, F> VarZeroVecComponents<'a, T, F> {
    #[inline]
    pub fn new() -> Self {
        Self {
            len: 0,
            indices: &[],
            things: &[],
            marker: PhantomData,
        }
    }
}
impl<'a, T: VarULE + ?Sized, F: VarZeroVecFormat> VarZeroVecComponents<'a, T, F> {
    /// Construct a new VarZeroVecComponents, checking invariants about the overall buffer size:
    ///
    /// - There must be either zero or at least four bytes (if four, this is the "length" parsed as a usize)
    /// - There must be at least `4*(length - 1) + 4` bytes total, to form the array `indices` of indices
    /// - `0..indices[0]` must index into a valid section of
    ///   `things` (the data after `indices`), such that it parses to a `T::VarULE`
    /// - `indices[i - 1]..indices[i]` must index into a valid section of
    ///   `things` (the data after `indices`), such that it parses to a `T::VarULE`
    /// - `indices[len - 2]..things.len()` must index into a valid section of
    ///   `things`, such that it parses to a `T::VarULE`
    #[inline]
    pub fn parse_bytes(slice: &'a [u8]) -> Result<Self, VarZeroVecFormatError> {
        // The empty VZV is special-cased to the empty slice
        if slice.is_empty() {
            return Ok(VarZeroVecComponents {
                len: 0,
                indices: &[],
                things: &[],
                marker: PhantomData,
            });
        }
        let len_bytes = slice
            .get(0..F::Len::SIZE)
            .ok_or(VarZeroVecFormatError::Metadata)?;
        let len_ule =
            F::Len::parse_bytes_to_slice(len_bytes).map_err(|_| VarZeroVecFormatError::Metadata)?;

        let len = len_ule
            .first()
            .ok_or(VarZeroVecFormatError::Metadata)?
            .iule_to_usize();

        let rest = slice
            .get(F::Len::SIZE..)
            .ok_or(VarZeroVecFormatError::Metadata)?;
        let len_u32 = u32::try_from(len).map_err(|_| VarZeroVecFormatError::Metadata);
        // We pass down the rest of the invariants
        Self::parse_bytes_with_length(len_u32?, rest)
    }

    /// Construct a new VarZeroVecComponents, checking invariants about the overall buffer size:
    ///
    /// - There must be at least `4*len` bytes total, to form the array `indices` of indices.
    /// - `indices[i]..indices[i+1]` must index into a valid section of
    ///   `things` (the data after `indices`), such that it parses to a `T::VarULE`
    /// - `indices[len - 1]..things.len()` must index into a valid section of
    ///   `things`, such that it parses to a `T::VarULE`
    #[inline]
    pub fn parse_bytes_with_length(
        len: u32,
        slice: &'a [u8],
    ) -> Result<Self, VarZeroVecFormatError> {
        let len_minus_one = len.checked_sub(1);
        // The empty VZV is special-cased to the empty slice
        let Some(len_minus_one) = len_minus_one else {
            return Ok(VarZeroVecComponents {
                len: 0,
                indices: &[],
                things: &[],
                marker: PhantomData,
            });
        };
        // The indices array is one element shorter since the first index is always 0,
        // so we use len_minus_one
        let indices_bytes = slice
            .get(..F::Index::SIZE * (len_minus_one as usize))
            .ok_or(VarZeroVecFormatError::Metadata)?;
        let things = slice
            .get(F::Index::SIZE * (len_minus_one as usize)..)
            .ok_or(VarZeroVecFormatError::Metadata)?;

        let borrowed = VarZeroVecComponents {
            len,
            indices: indices_bytes,
            things,
            marker: PhantomData,
        };

        borrowed.check_indices_and_things()?;

        Ok(borrowed)
    }

    /// Construct a [`VarZeroVecComponents`] from a byte slice that has previously
    /// successfully returned a [`VarZeroVecComponents`] when passed to
    /// [`VarZeroVecComponents::parse_bytes()`]. Will return the same
    /// object as one would get from calling [`VarZeroVecComponents::parse_bytes()`].
    ///
    /// # Safety
    /// The bytes must have previously successfully run through
    /// [`VarZeroVecComponents::parse_bytes()`]
    pub unsafe fn from_bytes_unchecked(slice: &'a [u8]) -> Self {
        // The empty VZV is special-cased to the empty slice
        if slice.is_empty() {
            return VarZeroVecComponents {
                len: 0,
                indices: &[],
                things: &[],
                marker: PhantomData,
            };
        }
        let (len_bytes, data_bytes) = unsafe { slice.split_at_unchecked(F::Len::SIZE) };
        // Safety: F::Len allows all byte sequences
        let len_ule = F::Len::slice_from_bytes_unchecked(len_bytes);

        let len = len_ule.get_unchecked(0).iule_to_usize();
        let len_u32 = len as u32;

        // Safety: This method requires the bytes to have passed through `parse_bytes()`
        // whereas we're calling something that asks for `parse_bytes_with_length()`.
        // The two methods perform similar validation, with parse_bytes() validating an additional
        // 4-byte `length` header.
        Self::from_bytes_unchecked_with_length(len_u32, data_bytes)
    }

    /// Construct a [`VarZeroVecComponents`] from a byte slice that has previously
    /// successfully returned a [`VarZeroVecComponents`] when passed to
    /// [`VarZeroVecComponents::parse_bytes()`]. Will return the same
    /// object as one would get from calling [`VarZeroVecComponents::parse_bytes()`].
    ///
    /// # Safety
    /// The len,bytes must have previously successfully run through
    /// [`VarZeroVecComponents::parse_bytes_with_length()`]
    pub unsafe fn from_bytes_unchecked_with_length(len: u32, slice: &'a [u8]) -> Self {
        let len_minus_one = len.checked_sub(1);
        // The empty VZV is special-cased to the empty slice
        let Some(len_minus_one) = len_minus_one else {
            return VarZeroVecComponents {
                len: 0,
                indices: &[],
                things: &[],
                marker: PhantomData,
            };
        };
        // The indices array is one element shorter since the first index is always 0,
        // so we use len_minus_one
        let indices_bytes = slice.get_unchecked(..F::Index::SIZE * (len_minus_one as usize));
        let things = slice.get_unchecked(F::Index::SIZE * (len_minus_one as usize)..);

        VarZeroVecComponents {
            len,
            indices: indices_bytes,
            things,
            marker: PhantomData,
        }
    }

    /// Get the number of elements in this vector
    #[inline]
    pub fn len(self) -> usize {
        self.len as usize
    }

    /// Returns `true` if the vector contains no elements.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Get the idx'th element out of this slice. Returns `None` if out of bounds.
    #[inline]
    pub fn get(self, idx: usize) -> Option<&'a T> {
        if idx >= self.len() {
            return None;
        }
        Some(unsafe { self.get_unchecked(idx) })
    }

    /// Get the idx'th element out of this slice. Does not bounds check.
    ///
    /// Safety:
    /// - `idx` must be in bounds (`idx < self.len()`)
    #[inline]
    pub(crate) unsafe fn get_unchecked(self, idx: usize) -> &'a T {
        let range = self.get_things_range(idx);
        let things_slice = self.things.get_unchecked(range);
        T::from_bytes_unchecked(things_slice)
    }

    /// Get the range in `things` for the element at `idx`. Does not bounds check.
    ///
    /// Safety:
    /// - `idx` must be in bounds (`idx < self.len()`)
    #[inline]
    pub(crate) unsafe fn get_things_range(self, idx: usize) -> Range<usize> {
        let start = if let Some(idx_minus_one) = idx.checked_sub(1) {
            self.indices_slice()
                .get_unchecked(idx_minus_one)
                .iule_to_usize()
        } else {
            0
        };
        let end = if idx + 1 == self.len() {
            self.things.len()
        } else {
            self.indices_slice().get_unchecked(idx).iule_to_usize()
        };
        debug_assert!(start <= end);
        start..end
    }

    /// Get the size, in bytes, of the indices array
    pub(crate) unsafe fn get_indices_size(self) -> usize {
        self.indices.len()
    }

    /// Check the internal invariants of VarZeroVecComponents:
    ///
    /// - `indices[i]..indices[i+1]` must index into a valid section of
    ///   `things`, such that it parses to a `T::VarULE`
    /// - `indices[len - 1]..things.len()` must index into a valid section of
    ///   `things`, such that it parses to a `T::VarULE`
    /// - `indices` is monotonically increasing
    ///
    /// This method is NOT allowed to call any other methods on VarZeroVecComponents since all other methods
    /// assume that the slice has been passed through check_indices_and_things
    #[inline]
    #[expect(clippy::len_zero)] // more explicit to enforce safety invariants
    fn check_indices_and_things(self) -> Result<(), VarZeroVecFormatError> {
        if self.len() == 0 {
            if self.things.len() > 0 {
                return Err(VarZeroVecFormatError::Metadata);
            } else {
                return Ok(());
            }
        }
        let indices_slice = self.indices_slice();
        assert_eq!(self.len(), indices_slice.len() + 1);
        // Safety: i is in bounds (assertion above)
        let mut start = 0;
        for i in 0..self.len() {
            // The indices array is offset by 1: indices[0] is the end of the first
            // element and the start of the next, since the start of the first element
            // is always things[0]. So to get the end we get element `i`.
            let end = if let Some(end) = indices_slice.get(i) {
                end.iule_to_usize()
            } else {
                // This only happens at i = self.len() - 1 = indices_slice.len() + 1 - 1
                // = indices_slice.len(). This is the last `end`, which is always the size of
                // `things` and thus never stored in the array
                self.things.len()
            };

            if start > end {
                return Err(VarZeroVecFormatError::Metadata);
            }
            if end > self.things.len() {
                return Err(VarZeroVecFormatError::Metadata);
            }
            // Safety: start..end is a valid range in self.things
            let bytes = unsafe { self.things.get_unchecked(start..end) };
            T::parse_bytes(bytes).map_err(VarZeroVecFormatError::Values)?;
            start = end;
        }
        Ok(())
    }

    /// Create an iterator over the Ts contained in VarZeroVecComponents
    #[inline]
    pub fn iter(self) -> VarZeroSliceIter<'a, T, F> {
        VarZeroSliceIter::new(self)
    }

    #[cfg(feature = "alloc")]
    pub fn to_vec(self) -> alloc::vec::Vec<alloc::boxed::Box<T>> {
        self.iter().map(T::to_boxed).collect()
    }

    #[inline]
    fn indices_slice(&self) -> &'a [F::Index] {
        unsafe { F::Index::slice_from_bytes_unchecked(self.indices) }
    }

    // Dump a debuggable representation of this type
    #[allow(unused)] // useful for debugging
    #[cfg(feature = "alloc")]
    pub(crate) fn dump(&self) -> alloc::string::String {
        let indices = self
            .indices_slice()
            .iter()
            .copied()
            .map(IntegerULE::iule_to_usize)
            .collect::<alloc::vec::Vec<_>>();
        alloc::format!("VarZeroVecComponents {{ indices: {indices:?} }}")
    }
}

/// An iterator over VarZeroSlice
#[derive(Debug)]
pub struct VarZeroSliceIter<'a, T: ?Sized, F = Index16> {
    components: VarZeroVecComponents<'a, T, F>,
    index: usize,
    // Safety invariant: must be a valid index into the data segment of `components`, or an index at the end
    // i.e. start_index <= components.things.len()
    //
    // It must be a valid index into the `things` array of components, coming from `components.indices_slice()`
    start_index: usize,
}

impl<'a, T: VarULE + ?Sized, F: VarZeroVecFormat> VarZeroSliceIter<'a, T, F> {
    fn new(c: VarZeroVecComponents<'a, T, F>) -> Self {
        Self {
            components: c,
            index: 0,
            // Invariant upheld, 0 is always a valid index-or-end
            start_index: 0,
        }
    }
}
impl<'a, T: VarULE + ?Sized, F: VarZeroVecFormat> Iterator for VarZeroSliceIter<'a, T, F> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        // Note: the indices array doesn't contain 0 or len, we need to specially handle those edges. The 0 is handled
        // by start_index, and the len is handled by the code for `end`.

        if self.index >= self.components.len() {
            return None;
        }

        // Invariant established: self.index is in bounds for self.components.len(),
        // which means it is in bounds for self.components.indices_slice() since that has the same length

        let end = if self.index + 1 == self.components.len() {
            // We don't store the end index since it is computable, so the last element should use self.components.things.len()
            self.components.things.len()
        } else {
            // Safety: self.index was known to be in bounds from the bounds check above.
            unsafe {
                self.components
                    .indices_slice()
                    .get_unchecked(self.index)
                    .iule_to_usize()
            }
        };
        // Invariant established: end has the same invariant as self.start_index since it comes from indices_slice, which is guaranteed
        // to only contain valid indexes

        let item = unsafe {
            // Safety: self.start_index and end both have in-range invariants, plus they are valid indices from indices_slice
            // which means we can treat this data as a T
            T::from_bytes_unchecked(self.components.things.get_unchecked(self.start_index..end))
        };
        self.index += 1;
        // Invariant upheld: end has the same invariant as self.start_index
        self.start_index = end;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remainder = self.components.len() - self.index;
        (remainder, Some(remainder))
    }
}

impl<'a, T: VarULE + ?Sized, F: VarZeroVecFormat> ExactSizeIterator for VarZeroSliceIter<'a, T, F> {
    fn len(&self) -> usize {
        self.components.len() - self.index
    }
}

impl<'a, T, F> VarZeroVecComponents<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    T: Ord,
    F: VarZeroVecFormat,
{
    /// Binary searches a sorted `VarZeroVecComponents<T>` for the given element. For more information, see
    /// the primitive function [`binary_search`](slice::binary_search).
    pub fn binary_search(&self, needle: &T) -> Result<usize, usize> {
        self.binary_search_by(|probe| probe.cmp(needle))
    }

    pub fn binary_search_in_range(
        &self,
        needle: &T,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        self.binary_search_in_range_by(|probe| probe.cmp(needle), range)
    }
}

impl<'a, T, F> VarZeroVecComponents<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    F: VarZeroVecFormat,
{
    /// Binary searches a sorted `VarZeroVecComponents<T>` for the given predicate. For more information, see
    /// the primitive function [`binary_search_by`](slice::binary_search_by).
    pub fn binary_search_by(&self, predicate: impl FnMut(&T) -> Ordering) -> Result<usize, usize> {
        // Safety: 0 and len are in range
        unsafe { self.binary_search_in_range_unchecked(predicate, 0..self.len()) }
    }

    // Binary search within a range.
    // Values returned are relative to the range start!
    pub fn binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        if range.end > self.len() {
            return None;
        }
        if range.end < range.start {
            return None;
        }
        // Safety: We bounds checked above: end is in-bounds or len, and start is <= end
        let range_absolute =
            unsafe { self.binary_search_in_range_unchecked(predicate, range.clone()) };
        // The values returned are relative to the range start
        Some(
            range_absolute
                .map(|o| o - range.start)
                .map_err(|e| e - range.start),
        )
    }

    /// Safety: range must be in range for the slice (start <= len, end <= len, start <= end)
    unsafe fn binary_search_in_range_unchecked(
        &self,
        mut predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Result<usize, usize> {
        // Function invariant: size is always end - start
        let mut start = range.start;
        let mut end = range.end;
        let mut size;

        // Loop invariant: 0 <= start < end <= len
        // This invariant is initialized by the function safety invariants and the loop condition
        while start < end {
            size = end - start;
            // This establishes mid < end (which implies mid < len)
            // size is end - start. start + size is end (which is <= len).
            // mid = start + size/2 will be less than end
            let mid = start + size / 2;

            // Safety: mid is < end <= len, so in-range
            let cmp = predicate(self.get_unchecked(mid));

            match cmp {
                Ordering::Less => {
                    // This retains the loop invariant since it
                    // increments start, and we already have 0 <= start
                    // start < end is enforced by the loop condition
                    start = mid + 1;
                }
                Ordering::Greater => {
                    // mid < end, so this decreases end.
                    // This means end <= len is still true, and
                    // end > start is enforced by the loop condition
                    end = mid;
                }
                Ordering::Equal => return Ok(mid),
            }
        }
        Err(start)
    }
}

/// Collects the bytes for a VarZeroSlice into a Vec.
#[cfg(feature = "alloc")]
pub fn get_serializable_bytes_non_empty<T, A, F>(elements: &[A]) -> Option<alloc::vec::Vec<u8>>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    debug_assert!(!elements.is_empty());
    let len = compute_serializable_len::<T, A, F>(elements)?;
    debug_assert!(
        len >= F::Len::SIZE as u32,
        "Must have at least F::Len::SIZE bytes to hold the length of the vector"
    );
    let mut output = alloc::vec![0u8; len as usize];
    write_serializable_bytes::<T, A, F>(elements, &mut output);
    Some(output)
}

/// Writes the bytes for a VarZeroLengthlessSlice into an output buffer.
/// Usable for a VarZeroSlice if you first write the length bytes.
///
/// Every byte in the buffer will be initialized after calling this function.
///
/// # Panics
///
/// Panics if the buffer is not exactly the correct length.
pub fn write_serializable_bytes_without_length<T, A, F>(elements: &[A], output: &mut [u8])
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    assert!(elements.len() <= F::Len::MAX_VALUE as usize);
    if elements.is_empty() {
        return;
    }

    // idx_offset = offset from the start of the buffer for the next index
    let mut idx_offset: usize = 0;
    // first_dat_offset = offset from the start of the buffer of the first data block
    let first_dat_offset: usize = idx_offset + (elements.len() - 1) * F::Index::SIZE;
    // dat_offset = offset from the start of the buffer of the next data block
    let mut dat_offset: usize = first_dat_offset;

    for (i, element) in elements.iter().enumerate() {
        let element_len = element.encode_var_ule_len();

        // The first index is always 0. We don't write it, or update the idx offset.
        if i != 0 {
            let idx_limit = idx_offset + F::Index::SIZE;
            #[expect(clippy::indexing_slicing)] // Function contract allows panicky behavior
            let idx_slice = &mut output[idx_offset..idx_limit];
            // VZV expects data offsets to be stored relative to the first data block
            let idx = dat_offset - first_dat_offset;
            assert!(idx <= F::Index::MAX_VALUE as usize);
            #[expect(clippy::expect_used)] // this function is explicitly panicky
            let bytes_to_write = F::Index::iule_from_usize(idx).expect(F::Index::TOO_LARGE_ERROR);
            idx_slice.copy_from_slice(ULE::slice_as_bytes(&[bytes_to_write]));

            idx_offset = idx_limit;
        }

        let dat_limit = dat_offset + element_len;
        #[expect(clippy::indexing_slicing)] // Function contract allows panicky behavior
        let dat_slice = &mut output[dat_offset..dat_limit];
        element.encode_var_ule_write(dat_slice);
        debug_assert_eq!(T::validate_bytes(dat_slice), Ok(()));
        dat_offset = dat_limit;
    }

    debug_assert_eq!(idx_offset, F::Index::SIZE * (elements.len() - 1));
    assert_eq!(dat_offset, output.len());
}

/// Writes the bytes for a VarZeroSlice into an output buffer.
///
/// Every byte in the buffer will be initialized after calling this function.
///
/// # Panics
///
/// Panics if the buffer is not exactly the correct length.
pub fn write_serializable_bytes<T, A, F>(elements: &[A], output: &mut [u8])
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    if elements.is_empty() {
        return;
    }
    assert!(elements.len() <= F::Len::MAX_VALUE as usize);
    #[expect(clippy::expect_used)] // This function is explicitly panicky
    let num_elements_ule = F::Len::iule_from_usize(elements.len()).expect(F::Len::TOO_LARGE_ERROR);
    #[expect(clippy::indexing_slicing)] // Function contract allows panicky behavior
    output[0..F::Len::SIZE].copy_from_slice(ULE::slice_as_bytes(&[num_elements_ule]));

    #[expect(clippy::indexing_slicing)] // Function contract allows panicky behavior
    write_serializable_bytes_without_length::<T, A, F>(elements, &mut output[F::Len::SIZE..]);
}

pub fn compute_serializable_len_without_length<T, A, F>(elements: &[A]) -> Option<u32>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    let elements_len = elements.len();
    let Some(elements_len_minus_one) = elements_len.checked_sub(1) else {
        // Empty vec is optimized to an empty byte representation
        return Some(0);
    };
    let idx_len: u32 = u32::try_from(elements_len_minus_one)
        .ok()?
        .checked_mul(F::Index::SIZE as u32)?;
    let data_len: u32 = elements
        .iter()
        .map(|v| u32::try_from(v.encode_var_ule_len()).ok())
        .try_fold(0u32, |s, v| s.checked_add(v?))?;
    let ret = idx_len.checked_add(data_len);
    if let Some(r) = ret {
        if r >= F::Index::MAX_VALUE {
            return None;
        }
    }
    ret
}

pub fn compute_serializable_len<T, A, F>(elements: &[A]) -> Option<u32>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    compute_serializable_len_without_length::<T, A, F>(elements).map(|x| x + F::Len::SIZE as u32)
}
