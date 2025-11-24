// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use core::cmp::Ordering;
use core::ops::Range;

/// A zero-copy "slice", i.e. the zero-copy version of `[T]`.
///
/// This behaves
/// similarly to [`ZeroVec<T>`], however [`ZeroVec<T>`] is allowed to contain
/// owned data and as such is ideal for deserialization since most human readable
/// serialization formats cannot unconditionally deserialize zero-copy.
///
/// This type can be used inside [`VarZeroVec<T>`](crate::VarZeroVec) and [`ZeroMap`](crate::ZeroMap):
/// This essentially allows for the construction of zero-copy types isomorphic to `Vec<Vec<T>>` by instead
/// using `VarZeroVec<ZeroSlice<T>>`. See the [`VarZeroVec`](crate::VarZeroVec) docs for an example.
///
/// # Examples
///
/// Const-construct a ZeroSlice of u16:
///
/// ```
/// use zerovec::ule::AsULE;
/// use zerovec::ZeroSlice;
///
/// const DATA: &ZeroSlice<u16> =
///     ZeroSlice::<u16>::from_ule_slice(&<u16 as AsULE>::ULE::from_array([
///         211, 281, 421, 32973,
///     ]));
///
/// assert_eq!(DATA.get(1), Some(281));
/// ```
#[repr(transparent)]
pub struct ZeroSlice<T: AsULE>([T::ULE]);

impl<T> ZeroSlice<T>
where
    T: AsULE,
{
    /// Returns an empty slice.
    pub const fn new_empty() -> &'static Self {
        Self::from_ule_slice(&[])
    }

    /// Get this [`ZeroSlice`] as a borrowed [`ZeroVec`]
    ///
    /// [`ZeroSlice`] does not have most of the methods that [`ZeroVec`] does,
    /// so it is recommended to convert it to a [`ZeroVec`] before doing anything.
    #[inline]
    pub const fn as_zerovec(&self) -> ZeroVec<'_, T> {
        ZeroVec::new_borrowed(&self.0)
    }

    /// Attempt to construct a `&ZeroSlice<T>` from a byte slice, returning an error
    /// if it's not a valid byte sequence
    pub fn parse_bytes(bytes: &[u8]) -> Result<&Self, UleError> {
        T::ULE::parse_bytes_to_slice(bytes).map(Self::from_ule_slice)
    }

    /// Uses a `&[u8]` buffer as a `ZeroVec<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`ZeroSlice::as_bytes()`].
    pub const unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // &[u8] and &[T::ULE] are the same slice with different length metadata.
        Self::from_ule_slice(core::slice::from_raw_parts(
            bytes.as_ptr() as *const T::ULE,
            bytes.len() / core::mem::size_of::<T::ULE>(),
        ))
    }

    /// Construct a `&ZeroSlice<T>` from a slice of ULEs.
    ///
    /// This function can be used for constructing ZeroVecs in a const context, avoiding
    /// parsing checks.
    ///
    /// See [`ZeroSlice`] for an example.
    #[inline]
    pub const fn from_ule_slice(slice: &[T::ULE]) -> &Self {
        // This is safe because ZeroSlice is transparent over [T::ULE]
        // so &ZeroSlice<T> can be safely cast from &[T::ULE]
        unsafe { &*(slice as *const _ as *const Self) }
    }

    /// Construct a `Box<ZeroSlice<T>>` from a boxed slice of ULEs
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn from_boxed_slice(slice: alloc::boxed::Box<[T::ULE]>) -> alloc::boxed::Box<Self> {
        // This is safe because ZeroSlice is transparent over [T::ULE]
        // so Box<ZeroSlice<T>> can be safely cast from Box<[T::ULE]>
        unsafe { alloc::boxed::Box::from_raw(alloc::boxed::Box::into_raw(slice) as *mut Self) }
    }

    /// Returns this slice as its underlying `&[u8]` byte buffer representation.
    ///
    /// Useful for serialization.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// // The little-endian bytes correspond to the numbers on the following line.
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let nums: &[u16] = &[211, 281, 421, 32973];
    ///
    /// let zerovec = ZeroVec::alloc_from_slice(nums);
    ///
    /// assert_eq!(bytes, zerovec.as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        T::ULE::slice_as_bytes(self.as_ule_slice())
    }

    /// Dereferences this slice as `&[T::ULE]`.
    #[inline]
    pub const fn as_ule_slice(&self) -> &[T::ULE] {
        &self.0
    }

    /// Returns the number of elements in this slice.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ule::AsULE;
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(4, zerovec.len());
    /// assert_eq!(
    ///     bytes.len(),
    ///     zerovec.len() * std::mem::size_of::<<u16 as AsULE>::ULE>()
    /// );
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.as_ule_slice().len()
    }

    /// Returns whether this slice is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_empty());
    ///
    /// let emptyvec: ZeroVec<u16> = ZeroVec::parse_bytes(&[]).expect("infallible");
    /// assert!(emptyvec.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.as_ule_slice().is_empty()
    }
}

impl<T> ZeroSlice<T>
where
    T: AsULE,
{
    /// Gets the element at the specified index. Returns `None` if out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(zerovec.get(2), Some(421));
    /// assert_eq!(zerovec.get(4), None);
    /// ```
    #[inline]
    pub fn get(&self, index: usize) -> Option<T> {
        self.as_ule_slice()
            .get(index)
            .copied()
            .map(T::from_unaligned)
    }

    /// Gets the entire slice as an array of length `N`. Returns `None` if the slice
    /// does not have exactly `N` elements.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// let array: [u16; 4] =
    ///     zerovec.get_as_array().expect("should be 4 items in array");
    ///
    /// assert_eq!(array[2], 421);
    /// ```
    pub fn get_as_array<const N: usize>(&self) -> Option<[T; N]> {
        let ule_array = <&[T::ULE; N]>::try_from(self.as_ule_slice()).ok()?;
        Some(ule_array.map(|u| T::from_unaligned(u)))
    }

    /// Gets a subslice of elements within a certain range. Returns `None` if the range
    /// is out of bounds of this `ZeroSlice`.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(
    ///     zerovec.get_subslice(1..3),
    ///     Some(&*ZeroVec::from_slice_or_alloc(&[0x0119, 0x01A5]))
    /// );
    /// assert_eq!(zerovec.get_subslice(3..5), None);
    /// ```
    #[inline]
    pub fn get_subslice(&self, range: Range<usize>) -> Option<&ZeroSlice<T>> {
        self.0.get(range).map(ZeroSlice::from_ule_slice)
    }

    /// Get a borrowed reference to the underlying ULE type at a specified index.
    ///
    /// Prefer [`Self::get()`] over this method where possible since working
    /// directly with `ULE` types is less ergonomic
    pub fn get_ule_ref(&self, index: usize) -> Option<&T::ULE> {
        self.as_ule_slice().get(index)
    }

    /// Casts a `ZeroSlice<T>` to a compatible `ZeroSlice<P>`.
    ///
    /// `T` and `P` are compatible if they have the same `ULE` representation.
    ///
    /// If the `ULE`s of `T` and `P` are different, use [`Self::try_as_converted()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroSlice;
    ///
    /// const BYTES: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// const ZS_U16: &ZeroSlice<u16> = {
    ///     match ZeroSlice::<u16>::try_from_bytes(BYTES) {
    ///         Ok(s) => s,
    ///         Err(_) => unreachable!(),
    ///     }
    /// };
    ///
    /// let zs_i16: &ZeroSlice<i16> = ZS_U16.cast();
    ///
    /// assert_eq!(ZS_U16.get(3), Some(32973));
    /// assert_eq!(zs_i16.get(3), Some(-32563));
    /// ```
    #[inline]
    pub const fn cast<P>(&self) -> &ZeroSlice<P>
    where
        P: AsULE<ULE = T::ULE>,
    {
        ZeroSlice::<P>::from_ule_slice(self.as_ule_slice())
    }

    /// Converts a `&ZeroSlice<T>` into a `&ZeroSlice<P>`.
    ///
    /// The resulting slice will have the same length as the original slice
    /// if and only if `T::ULE` and `P::ULE` are the same size.
    ///
    /// If `T` and `P` have the exact same `ULE`, use [`Self::cast()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroSlice;
    ///
    /// const BYTES: &[u8] = &[0x7F, 0xF3, 0x01, 0x00, 0x49, 0xF6, 0x01, 0x00];
    /// const ZS_U32: &ZeroSlice<u32> = {
    ///     match ZeroSlice::<u32>::try_from_bytes(BYTES) {
    ///         Ok(s) => s,
    ///         Err(_) => unreachable!(),
    ///     }
    /// };
    ///
    /// let zs_u8_4: &ZeroSlice<[u8; 4]> =
    ///     ZS_U32.try_as_converted().expect("valid code points");
    ///
    /// assert_eq!(ZS_U32.get(0), Some(127871));
    /// assert_eq!(zs_u8_4.get(0), Some([0x7F, 0xF3, 0x01, 0x00]));
    /// ```
    #[inline]
    pub fn try_as_converted<P: AsULE>(&self) -> Result<&ZeroSlice<P>, UleError> {
        let new_slice = P::ULE::parse_bytes_to_slice(self.as_bytes())?;
        Ok(ZeroSlice::from_ule_slice(new_slice))
    }

    /// Gets the first element. Returns `None` if empty.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(zerovec.first(), Some(211));
    /// ```
    #[inline]
    pub fn first(&self) -> Option<T> {
        self.as_ule_slice().first().copied().map(T::from_unaligned)
    }

    /// Gets the last element. Returns `None` if empty.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(zerovec.last(), Some(32973));
    /// ```
    #[inline]
    pub fn last(&self) -> Option<T> {
        self.as_ule_slice().last().copied().map(T::from_unaligned)
    }

    /// Gets an iterator over the elements.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// let mut it = zerovec.iter();
    ///
    /// assert_eq!(it.next(), Some(211));
    /// assert_eq!(it.next(), Some(281));
    /// assert_eq!(it.next(), Some(421));
    /// assert_eq!(it.next(), Some(32973));
    /// assert_eq!(it.next(), None);
    /// ```
    #[inline]
    pub fn iter<'a>(&'a self) -> ZeroSliceIter<'a, T> {
        ZeroSliceIter(self.as_ule_slice().iter())
    }

    /// Returns a tuple with the first element and a subslice of the remaining elements.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ule::AsULE;
    /// use zerovec::ZeroSlice;
    ///
    /// const DATA: &ZeroSlice<u16> =
    ///     ZeroSlice::<u16>::from_ule_slice(&<u16 as AsULE>::ULE::from_array([
    ///         211, 281, 421, 32973,
    ///     ]));
    /// const EXPECTED_VALUE: (u16, &ZeroSlice<u16>) = (
    ///     211,
    ///     ZeroSlice::<u16>::from_ule_slice(&<u16 as AsULE>::ULE::from_array([
    ///         281, 421, 32973,
    ///     ])),
    /// );
    /// assert_eq!(EXPECTED_VALUE, DATA.split_first().unwrap());
    /// ```
    #[inline]
    pub fn split_first(&self) -> Option<(T, &ZeroSlice<T>)> {
        if let Some(first) = self.first() {
            return Some((
                first,
                // `unwrap()` must succeed, because `first()` returned `Some`.
                #[expect(clippy::unwrap_used)]
                self.get_subslice(1..self.len()).unwrap(),
            ));
        }
        None
    }
}

/// An iterator over elements in a VarZeroVec
#[derive(Debug)]
pub struct ZeroSliceIter<'a, T: AsULE>(core::slice::Iter<'a, T::ULE>);

impl<'a, T: AsULE> Iterator for ZeroSliceIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.next().copied().map(T::from_unaligned)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, T: AsULE> ExactSizeIterator for ZeroSliceIter<'a, T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a, T: AsULE> DoubleEndedIterator for ZeroSliceIter<'a, T> {
    fn next_back(&mut self) -> Option<T> {
        self.0.next_back().copied().map(T::from_unaligned)
    }
}

impl<T> ZeroSlice<T>
where
    T: AsULE + Ord,
{
    /// Binary searches a sorted `ZeroVec<T>` for the given element. For more information, see
    /// the primitive function [`binary_search`].
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(zerovec.binary_search(&281), Ok(1));
    /// assert_eq!(zerovec.binary_search(&282), Err(2));
    /// ```
    ///
    /// [`binary_search`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search
    #[inline]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize> {
        self.as_ule_slice()
            .binary_search_by(|probe| T::from_unaligned(*probe).cmp(x))
    }
}

impl<T> ZeroSlice<T>
where
    T: AsULE,
{
    /// Binary searches a sorted `ZeroVec<T>` based on a given predicate. For more information, see
    /// the primitive function [`binary_search_by`].
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert_eq!(zerovec.binary_search_by(|x| x.cmp(&281)), Ok(1));
    /// assert_eq!(zerovec.binary_search_by(|x| x.cmp(&282)), Err(2));
    /// ```
    ///
    /// [`binary_search_by`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search_by
    #[inline]
    pub fn binary_search_by(
        &self,
        mut predicate: impl FnMut(T) -> Ordering,
    ) -> Result<usize, usize> {
        self.as_ule_slice()
            .binary_search_by(|probe| predicate(T::from_unaligned(*probe)))
    }
}

// Safety (based on the safety checklist on the VarULE trait):
// (`ZeroSlice<T>` is a transparent wrapper around [T::ULE])
//  1. [T::ULE] does not include any uninitialized or padding bytes (achieved by being a slice of a ULE type)
//  2. [T::ULE] is aligned to 1 byte (achieved by being a slice of a ULE type)
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//  6. `as_bytes()` and `parse_bytes()` are defaulted
//  7. `[T::ULE]` byte equality is semantic equality (relying on the guideline of the underlying `ULE` type)
unsafe impl<T: AsULE + 'static> VarULE for ZeroSlice<T> {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        T::ULE::validate_bytes(bytes)
    }

    #[inline]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        Self::from_ule_slice(T::ULE::slice_from_bytes_unchecked(bytes))
    }
}

impl<T> Eq for ZeroSlice<T> where T: AsULE + Eq {}

impl<T> PartialEq<ZeroSlice<T>> for ZeroSlice<T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &ZeroSlice<T>) -> bool {
        self.as_zerovec().eq(&other.as_zerovec())
    }
}

impl<T> PartialEq<[T]> for ZeroSlice<T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &[T]) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

impl<'a, T> PartialEq<ZeroVec<'a, T>> for ZeroSlice<T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &ZeroVec<'a, T>) -> bool {
        self.as_zerovec().eq(other)
    }
}

impl<'a, T> PartialEq<ZeroSlice<T>> for ZeroVec<'a, T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &ZeroSlice<T>) -> bool {
        self.eq(&other.as_zerovec())
    }
}

impl<T> fmt::Debug for ZeroSlice<T>
where
    T: AsULE + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_zerovec().fmt(f)
    }
}

impl<T: AsULE + PartialOrd> PartialOrd for ZeroSlice<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: AsULE + Ord> Ord for ZeroSlice<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

#[cfg(feature = "alloc")]
impl<T: AsULE> AsRef<ZeroSlice<T>> for alloc::vec::Vec<T::ULE> {
    fn as_ref(&self) -> &ZeroSlice<T> {
        ZeroSlice::<T>::from_ule_slice(self)
    }
}

impl<T: AsULE> AsRef<ZeroSlice<T>> for &[T::ULE] {
    fn as_ref(&self) -> &ZeroSlice<T> {
        ZeroSlice::<T>::from_ule_slice(self)
    }
}

impl<T> Default for &ZeroSlice<T>
where
    T: AsULE,
{
    fn default() -> Self {
        ZeroSlice::from_ule_slice(&[])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::zeroslice;

    #[test]
    fn test_split_first() {
        {
            // empty slice.
            assert_eq!(None, ZeroSlice::<u16>::new_empty().split_first());
        }
        {
            // single element slice
            const DATA: &ZeroSlice<u16> =
                zeroslice!(u16; <u16 as AsULE>::ULE::from_unsigned; [211]);
            assert_eq!((211, zeroslice![]), DATA.split_first().unwrap());
        }
        {
            // slice with many elements.
            const DATA: &ZeroSlice<u16> =
                zeroslice!(u16; <u16 as AsULE>::ULE::from_unsigned; [211, 281, 421, 32973]);
            const EXPECTED_VALUE: (u16, &ZeroSlice<u16>) = (
                211,
                zeroslice!(u16; <u16 as AsULE>::ULE::from_unsigned; [281, 421, 32973]),
            );

            assert_eq!(EXPECTED_VALUE, DATA.split_first().unwrap());
        }
    }
}
