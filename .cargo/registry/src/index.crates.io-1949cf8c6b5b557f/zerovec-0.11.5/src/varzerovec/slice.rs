// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::components::{VarZeroSliceIter, VarZeroVecComponents};
use super::vec::VarZeroVecInner;
use super::*;
use crate::ule::*;
use core::cmp::{Ord, Ordering, PartialOrd};
use core::fmt;
use core::marker::PhantomData;
use core::mem;

use core::ops::Index;
use core::ops::Range;

/// A zero-copy "slice", that works for unsized types, i.e. the zero-copy version of `[T]`
/// where `T` is not `Sized`.
///
/// This behaves similarly to [`VarZeroVec<T>`], however [`VarZeroVec<T>`] is allowed to contain
/// owned data and as such is ideal for deserialization since most human readable
/// serialization formats cannot unconditionally deserialize zero-copy.
///
/// This type can be used inside [`VarZeroVec<T>`](crate::VarZeroVec) and [`ZeroMap`](crate::ZeroMap):
/// This essentially allows for the construction of zero-copy types isomorphic to `Vec<Vec<T>>` by instead
/// using `VarZeroVec<ZeroSlice<T>>`.
///
/// The `F` type parameter is a [`VarZeroVecFormat`] (see its docs for more details), which can be used to select the
/// precise format of the backing buffer with various size and performance tradeoffs. It defaults to [`Index16`].
///
/// This type can be nested within itself to allow for multi-level nested `Vec`s.
///
/// # Examples
///
/// ## Nested Slices
///
/// The following code constructs the conceptual zero-copy equivalent of `Vec<Vec<Vec<str>>>`
///
/// ```rust
/// use zerovec::{VarZeroSlice, VarZeroVec};
/// let strings_1: Vec<&str> = vec!["foo", "bar", "baz"];
/// let strings_2: Vec<&str> = vec!["twelve", "seventeen", "forty two"];
/// let strings_3: Vec<&str> = vec!["我", "喜歡", "烏龍茶"];
/// let strings_4: Vec<&str> = vec!["w", "ω", "文", "𑄃"];
/// let strings_12 = vec![&*strings_1, &*strings_2];
/// let strings_34 = vec![&*strings_3, &*strings_4];
/// let all_strings = vec![strings_12, strings_34];
///
/// let vzv_1: VarZeroVec<str> = VarZeroVec::from(&strings_1);
/// let vzv_2: VarZeroVec<str> = VarZeroVec::from(&strings_2);
/// let vzv_3: VarZeroVec<str> = VarZeroVec::from(&strings_3);
/// let vzv_4: VarZeroVec<str> = VarZeroVec::from(&strings_4);
/// let vzv_12 = VarZeroVec::from(&[vzv_1.as_slice(), vzv_2.as_slice()]);
/// let vzv_34 = VarZeroVec::from(&[vzv_3.as_slice(), vzv_4.as_slice()]);
/// let vzv_all = VarZeroVec::from(&[vzv_12.as_slice(), vzv_34.as_slice()]);
///
/// let reconstructed: Vec<Vec<Vec<String>>> = vzv_all
///     .iter()
///     .map(|v: &VarZeroSlice<VarZeroSlice<str>>| {
///         v.iter()
///             .map(|x: &VarZeroSlice<_>| {
///                 x.as_varzerovec()
///                     .iter()
///                     .map(|s| s.to_owned())
///                     .collect::<Vec<String>>()
///             })
///             .collect::<Vec<_>>()
///     })
///     .collect::<Vec<_>>();
/// assert_eq!(reconstructed, all_strings);
///
/// let bytes = vzv_all.as_bytes();
/// let vzv_from_bytes: VarZeroVec<VarZeroSlice<VarZeroSlice<str>>> =
///     VarZeroVec::parse_bytes(bytes).unwrap();
/// assert_eq!(vzv_from_bytes, vzv_all);
/// ```
///
/// ## Iterate over Windows
///
/// Although [`VarZeroSlice`] does not itself have a `.windows` iterator like
/// [core::slice::Windows], this behavior can be easily modeled using an iterator:
///
/// ```
/// use zerovec::VarZeroVec;
///
/// let vzv = VarZeroVec::<str>::from(&["a", "b", "c", "d"]);
/// # let mut pairs: Vec<(&str, &str)> = Vec::new();
///
/// let mut it = vzv.iter().peekable();
/// while let (Some(x), Some(y)) = (it.next(), it.peek()) {
///     // Evaluate (x, y) here.
/// #   pairs.push((x, y));
/// }
/// # assert_eq!(pairs, &[("a", "b"), ("b", "c"), ("c", "d")]);
/// ```
//
// safety invariant: The slice MUST be one which parses to
// a valid VarZeroVecComponents<T>
#[repr(transparent)]
pub struct VarZeroSlice<T: ?Sized, F = Index16> {
    marker: PhantomData<(F, T)>,
    /// The original slice this was constructed from
    entire_slice: [u8],
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> VarZeroSlice<T, F> {
    /// Construct a new empty VarZeroSlice
    pub const fn new_empty() -> &'static Self {
        // The empty VZV is special-cased to the empty slice
        unsafe { mem::transmute(&[] as &[u8]) }
    }

    /// Obtain a [`VarZeroVecComponents`] borrowing from the internal buffer
    #[inline]
    pub(crate) fn as_components<'a>(&'a self) -> VarZeroVecComponents<'a, T, F> {
        unsafe {
            // safety: VarZeroSlice is guaranteed to parse here
            VarZeroVecComponents::from_bytes_unchecked(&self.entire_slice)
        }
    }

    /// Uses a `&[u8]` buffer as a `VarZeroSlice<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`VarZeroSlice::as_bytes()`].
    pub const unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // self is really just a wrapper around a byte slice
        mem::transmute(bytes)
    }

    /// Get the number of elements in this slice
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vec.len(), 4);
    /// ```
    pub fn len(&self) -> usize {
        self.as_components().len()
    }

    /// Returns `true` if the slice contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings: Vec<String> = vec![];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert!(vec.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.as_components().is_empty()
    }

    /// Obtain an iterator over this slice's elements
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// let mut iter_results: Vec<&str> = vec.iter().collect();
    /// assert_eq!(iter_results[0], "foo");
    /// assert_eq!(iter_results[1], "bar");
    /// assert_eq!(iter_results[2], "baz");
    /// assert_eq!(iter_results[3], "quux");
    /// ```
    pub fn iter<'b>(&'b self) -> VarZeroSliceIter<'b, T, F> {
        self.as_components().iter()
    }

    /// Get one of this slice's elements, returning `None` if the index is out of bounds
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// let mut iter_results: Vec<&str> = vec.iter().collect();
    /// assert_eq!(vec.get(0), Some("foo"));
    /// assert_eq!(vec.get(1), Some("bar"));
    /// assert_eq!(vec.get(2), Some("baz"));
    /// assert_eq!(vec.get(3), Some("quux"));
    /// assert_eq!(vec.get(4), None);
    /// ```
    pub fn get(&self, idx: usize) -> Option<&T> {
        self.as_components().get(idx)
    }

    /// Get one of this slice's elements
    ///
    /// # Safety
    ///
    /// `index` must be in range
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// let mut iter_results: Vec<&str> = vec.iter().collect();
    /// unsafe {
    ///     assert_eq!(vec.get_unchecked(0), "foo");
    ///     assert_eq!(vec.get_unchecked(1), "bar");
    ///     assert_eq!(vec.get_unchecked(2), "baz");
    ///     assert_eq!(vec.get_unchecked(3), "quux");
    /// }
    /// ```
    pub unsafe fn get_unchecked(&self, idx: usize) -> &T {
        self.as_components().get_unchecked(idx)
    }

    /// Obtain an owned `Vec<Box<T>>` out of this
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> alloc::vec::Vec<alloc::boxed::Box<T>> {
        self.as_components().to_vec()
    }

    /// Get a reference to the entire encoded backing buffer of this slice
    ///
    /// The bytes can be passed back to [`Self::parse_bytes()`].
    ///
    /// To take the bytes as a vector, see [`VarZeroVec::into_bytes()`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz"];
    /// let vzv = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vzv, VarZeroVec::parse_bytes(vzv.as_bytes()).unwrap());
    /// ```
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.entire_slice
    }

    /// Get this [`VarZeroSlice`] as a borrowed [`VarZeroVec`]
    ///
    /// If you wish to repeatedly call methods on this [`VarZeroSlice`],
    /// it is more efficient to perform this conversion first
    pub const fn as_varzerovec<'a>(&'a self) -> VarZeroVec<'a, T, F> {
        VarZeroVec(VarZeroVecInner::Borrowed(self))
    }

    /// Parse a VarZeroSlice from a slice of the appropriate format
    ///
    /// Slices of the right format can be obtained via [`VarZeroSlice::as_bytes()`]
    pub fn parse_bytes<'a>(slice: &'a [u8]) -> Result<&'a Self, UleError> {
        <Self as VarULE>::parse_bytes(slice)
    }
}

impl<T, F> VarZeroSlice<T, F>
where
    T: VarULE,
    T: ?Sized,
    T: Ord,
    F: VarZeroVecFormat,
{
    /// Binary searches a sorted `VarZeroVec<T>` for the given element. For more information, see
    /// the standard library function [`binary_search`].
    ///
    /// # Example
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["a", "b", "f", "g"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vec.binary_search("f"), Ok(2));
    /// assert_eq!(vec.binary_search("e"), Err(2));
    /// ```
    ///
    /// [`binary_search`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search
    #[inline]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize> {
        self.as_components().binary_search(x)
    }

    /// Binary searches a `VarZeroVec<T>` for the given element within a certain sorted range.
    ///
    /// If the range is out of bounds, returns `None`. Otherwise, returns a `Result` according
    /// to the behavior of the standard library function [`binary_search`].
    ///
    /// The index is returned relative to the start of the range.
    ///
    /// # Example
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    /// let strings = vec!["a", "b", "f", "g", "m", "n", "q"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// // Same behavior as binary_search when the range covers the whole slice:
    /// assert_eq!(vec.binary_search_in_range("g", 0..7), Some(Ok(3)));
    /// assert_eq!(vec.binary_search_in_range("h", 0..7), Some(Err(4)));
    ///
    /// // Will not look outside of the range:
    /// assert_eq!(vec.binary_search_in_range("g", 0..1), Some(Err(1)));
    /// assert_eq!(vec.binary_search_in_range("g", 6..7), Some(Err(0)));
    ///
    /// // Will return indices relative to the start of the range:
    /// assert_eq!(vec.binary_search_in_range("g", 1..6), Some(Ok(2)));
    /// assert_eq!(vec.binary_search_in_range("h", 1..6), Some(Err(3)));
    ///
    /// // Will return `None` if the range is out of bounds:
    /// assert_eq!(vec.binary_search_in_range("x", 100..200), None);
    /// assert_eq!(vec.binary_search_in_range("x", 0..200), None);
    /// ```
    ///
    /// [`binary_search`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search
    #[inline]
    pub fn binary_search_in_range(
        &self,
        x: &T,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        self.as_components().binary_search_in_range(x, range)
    }
}

impl<T, F> VarZeroSlice<T, F>
where
    T: VarULE,
    T: ?Sized,
    F: VarZeroVecFormat,
{
    /// Binary searches a sorted `VarZeroVec<T>` for the given predicate. For more information, see
    /// the standard library function [`binary_search_by`].
    ///
    /// # Example
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    /// let strings = vec!["a", "b", "f", "g"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vec.binary_search_by(|probe| probe.cmp("f")), Ok(2));
    /// assert_eq!(vec.binary_search_by(|probe| probe.cmp("e")), Err(2));
    /// ```
    ///
    /// [`binary_search_by`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search_by
    #[inline]
    pub fn binary_search_by(&self, predicate: impl FnMut(&T) -> Ordering) -> Result<usize, usize> {
        self.as_components().binary_search_by(predicate)
    }

    /// Binary searches a `VarZeroVec<T>` for the given predicate within a certain sorted range.
    ///
    /// If the range is out of bounds, returns `None`. Otherwise, returns a `Result` according
    /// to the behavior of the standard library function [`binary_search`].
    ///
    /// The index is returned relative to the start of the range.
    ///
    /// # Example
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    /// let strings = vec!["a", "b", "f", "g", "m", "n", "q"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// // Same behavior as binary_search when the range covers the whole slice:
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("g"), 0..7),
    ///     Some(Ok(3))
    /// );
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("h"), 0..7),
    ///     Some(Err(4))
    /// );
    ///
    /// // Will not look outside of the range:
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("g"), 0..1),
    ///     Some(Err(1))
    /// );
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("g"), 6..7),
    ///     Some(Err(0))
    /// );
    ///
    /// // Will return indices relative to the start of the range:
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("g"), 1..6),
    ///     Some(Ok(2))
    /// );
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("h"), 1..6),
    ///     Some(Err(3))
    /// );
    ///
    /// // Will return `None` if the range is out of bounds:
    /// assert_eq!(
    ///     vec.binary_search_in_range_by(|v| v.cmp("x"), 100..200),
    ///     None
    /// );
    /// assert_eq!(vec.binary_search_in_range_by(|v| v.cmp("x"), 0..200), None);
    /// ```
    ///
    /// [`binary_search`]: https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search
    pub fn binary_search_in_range_by(
        &self,
        predicate: impl FnMut(&T) -> Ordering,
        range: Range<usize>,
    ) -> Option<Result<usize, usize>> {
        self.as_components()
            .binary_search_in_range_by(predicate, range)
    }
}
// Safety (based on the safety checklist on the VarULE trait):
//  1. VarZeroSlice does not include any uninitialized or padding bytes (achieved by `#[repr(transparent)]` on a
//     `[u8]` slice which satisfies this invariant)
//  2. VarZeroSlice is aligned to 1 byte (achieved by `#[repr(transparent)]` on a
//     `[u8]` slice which satisfies this invariant)
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//  6. `as_bytes()` is equivalent to a regular transmute of the underlying data
//  7. VarZeroSlice byte equality is semantic equality (relying on the guideline of the underlying VarULE type)
unsafe impl<T: VarULE + ?Sized + 'static, F: VarZeroVecFormat> VarULE for VarZeroSlice<T, F> {
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        let _: VarZeroVecComponents<T, F> =
            VarZeroVecComponents::parse_bytes(bytes).map_err(|_| UleError::parse::<Self>())?;
        Ok(())
    }

    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // self is really just a wrapper around a byte slice
        mem::transmute(bytes)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.entire_slice
    }
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> Index<usize> for VarZeroSlice<T, F> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        #[expect(clippy::panic)] // documented
        match self.get(index) {
            Some(x) => x,
            None => panic!(
                "index out of bounds: the len is {} but the index is {index}",
                self.len()
            ),
        }
    }
}

impl<T, F> PartialEq<VarZeroSlice<T, F>> for VarZeroSlice<T, F>
where
    T: VarULE,
    T: ?Sized,
    T: PartialEq,
    F: VarZeroVecFormat,
{
    #[inline]
    fn eq(&self, other: &VarZeroSlice<T, F>) -> bool {
        // VarULE has an API guarantee that this is equivalent
        // to `T::VarULE::eq()`
        self.entire_slice.eq(&other.entire_slice)
    }
}

impl<T, F> Eq for VarZeroSlice<T, F>
where
    T: VarULE,
    T: ?Sized,
    T: Eq,
    F: VarZeroVecFormat,
{
}

impl<T: VarULE + ?Sized + PartialOrd, F: VarZeroVecFormat> PartialOrd for VarZeroSlice<T, F> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: VarULE + ?Sized + Ord, F: VarZeroVecFormat> Ord for VarZeroSlice<T, F> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> fmt::Debug for VarZeroSlice<T, F>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: ?Sized, F: VarZeroVecFormat> AsRef<VarZeroSlice<T, F>> for VarZeroSlice<T, F> {
    fn as_ref(&self) -> &VarZeroSlice<T, F> {
        self
    }
}
