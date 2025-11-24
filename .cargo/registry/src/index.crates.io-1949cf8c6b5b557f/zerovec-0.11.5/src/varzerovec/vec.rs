// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ule::*;

use core::cmp::{Ord, Ordering, PartialOrd};
use core::fmt;
use core::ops::Deref;

use super::*;

/// A zero-copy, byte-aligned vector for variable-width types.
///
/// `VarZeroVec<T>` is designed as a drop-in replacement for `Vec<T>` in situations where it is
/// desirable to borrow data from an unaligned byte slice, such as zero-copy deserialization, and
/// where `T`'s data is variable-length (e.g. `String`)
///
/// `T` must implement [`VarULE`], which is already implemented for [`str`] and `[u8]`. For storing more
/// complicated series of elements, it is implemented on `ZeroSlice<T>` as well as `VarZeroSlice<T>`
/// for nesting. [`zerovec::make_varule`](crate::make_varule) may be used to generate
/// a dynamically-sized [`VarULE`] type and conversions to and from a custom type.
///
/// For example, here are some owned types and their zero-copy equivalents:
///
/// - `Vec<String>`: `VarZeroVec<'a, str>`
/// - `Vec<Vec<u8>>>`: `VarZeroVec<'a, [u8]>`
/// - `Vec<Vec<u32>>`: `VarZeroVec<'a, ZeroSlice<u32>>`
/// - `Vec<Vec<String>>`: `VarZeroVec<'a, VarZeroSlice<str>>`
///
/// Most of the methods on `VarZeroVec<'a, T>` come from its [`Deref`] implementation to [`VarZeroSlice<T>`](VarZeroSlice).
///
/// For creating zero-copy vectors of fixed-size types, see [`ZeroVec`](crate::ZeroVec).
///
/// `VarZeroVec<T>` behaves much like [`Cow`](alloc::borrow::Cow), where it can be constructed from
/// owned data (and then mutated!) but can also borrow from some buffer.
///
/// The `F` type parameter is a [`VarZeroVecFormat`] (see its docs for more details), which can be used to select the
/// precise format of the backing buffer with various size and performance tradeoffs. It defaults to [`Index16`].
///
/// # Bytes and Equality
///
/// Two [`VarZeroVec`]s are equal if and only if their bytes are equal, as described in the trait
/// [`VarULE`]. However, we do not guarantee stability of byte equality or serialization format
/// across major SemVer releases.
///
/// To compare a [`Vec<T>`] to a [`VarZeroVec<T>`], it is generally recommended to use
/// [`Iterator::eq`], since it is somewhat expensive at runtime to convert from a [`Vec<T>`] to a
/// [`VarZeroVec<T>`] or vice-versa.
///
/// Prior to zerovec reaching 1.0, the precise byte representation of [`VarZeroVec`] is still
/// under consideration, with different options along the space-time spectrum. See
/// [#1410](https://github.com/unicode-org/icu4x/issues/1410).
///
/// # Example
///
/// ```rust
/// use zerovec::VarZeroVec;
///
/// // The little-endian bytes correspond to the list of strings.
/// let strings = vec!["w", "ω", "文", "𑄃"];
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Data<'a> {
///     #[serde(borrow)]
///     strings: VarZeroVec<'a, str>,
/// }
///
/// let data = Data {
///     strings: VarZeroVec::from(&strings),
/// };
///
/// let bincode_bytes =
///     bincode::serialize(&data).expect("Serialization should be successful");
///
/// // Will deserialize without allocations
/// let deserialized: Data = bincode::deserialize(&bincode_bytes)
///     .expect("Deserialization should be successful");
///
/// assert_eq!(deserialized.strings.get(2), Some("文"));
/// assert_eq!(deserialized.strings, &*strings);
/// ```
///
/// Here's another example with `ZeroSlice<T>` (similar to `[T]`):
///
/// ```rust
/// use zerovec::VarZeroVec;
/// use zerovec::ZeroSlice;
///
/// // The structured list correspond to the list of integers.
/// let numbers: &[&[u32]] = &[
///     &[12, 25, 38],
///     &[39179, 100],
///     &[42, 55555],
///     &[12345, 54321, 9],
/// ];
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Data<'a> {
///     #[serde(borrow)]
///     vecs: VarZeroVec<'a, ZeroSlice<u32>>,
/// }
///
/// let data = Data {
///     vecs: VarZeroVec::from(numbers),
/// };
///
/// let bincode_bytes =
///     bincode::serialize(&data).expect("Serialization should be successful");
///
/// let deserialized: Data = bincode::deserialize(&bincode_bytes)
///     .expect("Deserialization should be successful");
///
/// assert_eq!(deserialized.vecs[0].get(1).unwrap(), 25);
/// assert_eq!(deserialized.vecs[1], *numbers[1]);
/// ```
///
/// [`VarZeroVec`]s can be nested infinitely via a similar mechanism, see the docs of [`VarZeroSlice`]
/// for more information.
///
/// # How it Works
///
/// `VarZeroVec<T>`, when used with non-human-readable serializers (like `bincode`), will
/// serialize to a specially formatted list of bytes. The format is:
///
/// -  2 bytes for `length` (interpreted as a little-endian u16)
/// - `2 * (length - 1)` bytes of `indices` (interpreted as little-endian u16s)
/// - Remaining bytes for actual `data`
///
/// The format is tweakable by setting the `F` parameter, by default it uses u16 indices and lengths but other
/// `VarZeroVecFormat` types can set other sizes.
///
/// Each element in the `indices` array points to the ending index of its corresponding
/// data part in the `data` list. The starting index can be calculated from the ending index
/// of the next element (or 0 for the first element). The last ending index, not stored in the array, is
/// the length of the `data` segment.
///
/// See [the design doc](https://github.com/unicode-org/icu4x/blob/main/utils/zerovec/design_doc.md) for more details.
///
/// [`ule`]: crate::ule
pub struct VarZeroVec<'a, T: ?Sized, F = Index16>(pub(crate) VarZeroVecInner<'a, T, F>);

pub(crate) enum VarZeroVecInner<'a, T: ?Sized, F = Index16> {
    #[cfg(feature = "alloc")]
    Owned(VarZeroVecOwned<T, F>),
    Borrowed(&'a VarZeroSlice<T, F>),
}

impl<'a, T: ?Sized, F> Clone for VarZeroVec<'a, T, F> {
    fn clone(&self) -> Self {
        match self.0 {
            #[cfg(feature = "alloc")]
            VarZeroVecInner::Owned(ref o) => o.clone().into(),
            VarZeroVecInner::Borrowed(b) => b.into(),
        }
    }
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> fmt::Debug for VarZeroVec<'_, T, F>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        VarZeroSlice::fmt(self, f)
    }
}

#[cfg(feature = "alloc")]
impl<'a, T: ?Sized, F> From<VarZeroVecOwned<T, F>> for VarZeroVec<'a, T, F> {
    #[inline]
    fn from(other: VarZeroVecOwned<T, F>) -> Self {
        Self(VarZeroVecInner::Owned(other))
    }
}

impl<'a, T: ?Sized, F> From<&'a VarZeroSlice<T, F>> for VarZeroVec<'a, T, F> {
    fn from(other: &'a VarZeroSlice<T, F>) -> Self {
        Self(VarZeroVecInner::Borrowed(other))
    }
}

#[cfg(feature = "alloc")]
impl<'a, T: ?Sized + VarULE, F: VarZeroVecFormat> From<VarZeroVec<'a, T, F>>
    for VarZeroVecOwned<T, F>
{
    #[inline]
    fn from(other: VarZeroVec<'a, T, F>) -> Self {
        match other.0 {
            VarZeroVecInner::Owned(o) => o,
            VarZeroVecInner::Borrowed(b) => b.into(),
        }
    }
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> Default for VarZeroVec<'_, T, F> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> Deref for VarZeroVec<'_, T, F> {
    type Target = VarZeroSlice<T, F>;
    fn deref(&self) -> &VarZeroSlice<T, F> {
        self.as_slice()
    }
}

impl<'a, T: VarULE + ?Sized, F: VarZeroVecFormat> VarZeroVec<'a, T, F> {
    /// Creates a new, empty `VarZeroVec<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::VarZeroVec;
    ///
    /// let vzv: VarZeroVec<str> = VarZeroVec::new();
    /// assert!(vzv.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self(VarZeroVecInner::Borrowed(VarZeroSlice::new_empty()))
    }

    /// Parse a VarZeroVec from a slice of the appropriate format
    ///
    /// Slices of the right format can be obtained via [`VarZeroSlice::as_bytes()`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(&vec[0], "foo");
    /// assert_eq!(&vec[1], "bar");
    /// assert_eq!(&vec[2], "baz");
    /// assert_eq!(&vec[3], "quux");
    /// ```
    pub fn parse_bytes(slice: &'a [u8]) -> Result<Self, UleError> {
        let borrowed = VarZeroSlice::<T, F>::parse_bytes(slice)?;

        Ok(Self(VarZeroVecInner::Borrowed(borrowed)))
    }

    /// Uses a `&[u8]` buffer as a `VarZeroVec<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`VarZeroSlice::as_bytes()`].
    pub const unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        Self(VarZeroVecInner::Borrowed(core::mem::transmute::<
            &[u8],
            &VarZeroSlice<T, F>,
        >(bytes)))
    }

    /// Convert this into a mutable vector of the owned `T` type, cloning if necessary.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use zerovec::VarZeroVec;
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let mut vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vec.len(), 4);
    /// let mutvec = vec.make_mut();
    /// mutvec.push("lorem ipsum".into());
    /// mutvec[2] = "dolor sit".into();
    /// assert_eq!(&vec[0], "foo");
    /// assert_eq!(&vec[1], "bar");
    /// assert_eq!(&vec[2], "dolor sit");
    /// assert_eq!(&vec[3], "quux");
    /// assert_eq!(&vec[4], "lorem ipsum");
    /// ```
    //
    // This function is crate-public for now since we don't yet want to stabilize
    // the internal implementation details
    #[cfg(feature = "alloc")]
    pub fn make_mut(&mut self) -> &mut VarZeroVecOwned<T, F> {
        match self.0 {
            VarZeroVecInner::Owned(ref mut vec) => vec,
            VarZeroVecInner::Borrowed(slice) => {
                let new_self = VarZeroVecOwned::from_slice(slice);
                *self = new_self.into();
                // recursion is limited since we are guaranteed to hit the Owned branch
                self.make_mut()
            }
        }
    }

    /// Converts a borrowed ZeroVec to an owned ZeroVec. No-op if already owned.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz", "quux"];
    /// let vec = VarZeroVec::<str>::from(&strings);
    ///
    /// assert_eq!(vec.len(), 4);
    /// // has 'static lifetime
    /// let owned = vec.into_owned();
    /// ```
    #[cfg(feature = "alloc")]
    pub fn into_owned(mut self) -> VarZeroVec<'static, T, F> {
        self.make_mut();
        match self.0 {
            VarZeroVecInner::Owned(vec) => vec.into(),
            _ => unreachable!(),
        }
    }

    /// Obtain this `VarZeroVec` as a [`VarZeroSlice`]
    pub fn as_slice(&self) -> &VarZeroSlice<T, F> {
        match self.0 {
            #[cfg(feature = "alloc")]
            VarZeroVecInner::Owned(ref owned) => owned,
            VarZeroVecInner::Borrowed(b) => b,
        }
    }

    /// Takes the byte vector representing the encoded data of this VarZeroVec. If borrowed,
    /// this function allocates a byte vector and copies the borrowed bytes into it.
    ///
    /// The bytes can be passed back to [`Self::parse_bytes()`].
    ///
    /// To get a reference to the bytes without moving, see [`VarZeroSlice::as_bytes()`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```rust
    /// # use zerovec::VarZeroVec;
    ///
    /// let strings = vec!["foo", "bar", "baz"];
    /// let bytes = VarZeroVec::<str>::from(&strings).into_bytes();
    ///
    /// let mut borrowed: VarZeroVec<str> =
    ///     VarZeroVec::parse_bytes(&bytes).unwrap();
    /// assert_eq!(borrowed, &*strings);
    /// ```
    #[cfg(feature = "alloc")]
    pub fn into_bytes(self) -> alloc::vec::Vec<u8> {
        match self.0 {
            VarZeroVecInner::Owned(vec) => vec.into_bytes(),
            VarZeroVecInner::Borrowed(vec) => vec.as_bytes().to_vec(),
        }
    }

    /// Return whether the [`VarZeroVec`] is operating on owned or borrowed
    /// data. [`VarZeroVec::into_owned()`] and [`VarZeroVec::make_mut()`] can
    /// be used to force it into an owned type
    pub fn is_owned(&self) -> bool {
        match self.0 {
            #[cfg(feature = "alloc")]
            VarZeroVecInner::Owned(..) => true,
            VarZeroVecInner::Borrowed(..) => false,
        }
    }

    #[doc(hidden)]
    pub fn as_components<'b>(&'b self) -> VarZeroVecComponents<'b, T, F> {
        self.as_slice().as_components()
    }
}

#[cfg(feature = "alloc")]
impl<A, T, F> From<&alloc::vec::Vec<A>> for VarZeroVec<'static, T, F>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    #[inline]
    fn from(elements: &alloc::vec::Vec<A>) -> Self {
        Self::from(elements.as_slice())
    }
}

#[cfg(feature = "alloc")]
impl<A, T, F> From<&[A]> for VarZeroVec<'static, T, F>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    #[inline]
    fn from(elements: &[A]) -> Self {
        if elements.is_empty() {
            VarZeroSlice::new_empty().into()
        } else {
            #[expect(clippy::unwrap_used)] // TODO(#1410) Better story for fallibility
            VarZeroVecOwned::try_from_elements(elements).unwrap().into()
        }
    }
}

#[cfg(feature = "alloc")]
impl<A, T, F, const N: usize> From<&[A; N]> for VarZeroVec<'static, T, F>
where
    T: VarULE + ?Sized,
    A: EncodeAsVarULE<T>,
    F: VarZeroVecFormat,
{
    #[inline]
    fn from(elements: &[A; N]) -> Self {
        Self::from(elements.as_slice())
    }
}

impl<'a, 'b, T, F> PartialEq<VarZeroVec<'b, T, F>> for VarZeroVec<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    T: PartialEq,
    F: VarZeroVecFormat,
{
    #[inline]
    fn eq(&self, other: &VarZeroVec<'b, T, F>) -> bool {
        // VZV::from_elements used to produce a non-canonical representation of the
        // empty VZV, so we cannot use byte equality for empty vecs.
        if self.is_empty() || other.is_empty() {
            return self.is_empty() && other.is_empty();
        }
        // VarULE has an API guarantee that byte equality is semantic equality.
        // For non-empty VZVs, there's only a single metadata representation,
        // so this guarantee extends to the whole VZV representation.
        self.as_bytes().eq(other.as_bytes())
    }
}

impl<'a, T, F> Eq for VarZeroVec<'a, T, F>
where
    T: VarULE,
    T: ?Sized,
    T: Eq,
    F: VarZeroVecFormat,
{
}

impl<T, A, F> PartialEq<&'_ [A]> for VarZeroVec<'_, T, F>
where
    T: VarULE + ?Sized,
    T: PartialEq,
    A: AsRef<T>,
    F: VarZeroVecFormat,
{
    #[inline]
    fn eq(&self, other: &&[A]) -> bool {
        self.iter().eq(other.iter().map(|t| t.as_ref()))
    }
}

impl<T, A, F, const N: usize> PartialEq<[A; N]> for VarZeroVec<'_, T, F>
where
    T: VarULE + ?Sized,
    T: PartialEq,
    A: AsRef<T>,
    F: VarZeroVecFormat,
{
    #[inline]
    fn eq(&self, other: &[A; N]) -> bool {
        self.iter().eq(other.iter().map(|t| t.as_ref()))
    }
}

impl<'a, T: VarULE + ?Sized + PartialOrd, F: VarZeroVecFormat> PartialOrd for VarZeroVec<'a, T, F> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<'a, T: VarULE + ?Sized + Ord, F: VarZeroVecFormat> Ord for VarZeroVec<'a, T, F> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

#[test]
fn assert_single_empty_representation() {
    assert_eq!(
        VarZeroVec::<str>::new().as_bytes(),
        VarZeroVec::<str>::from(&[] as &[&str]).as_bytes()
    );

    use crate::map::MutableZeroVecLike;
    let mut vzv = VarZeroVec::<str>::from(&["hello", "world"][..]);
    assert_eq!(vzv.len(), 2);
    assert!(!vzv.as_bytes().is_empty());
    vzv.zvl_remove(0);
    assert_eq!(vzv.len(), 1);
    assert!(!vzv.as_bytes().is_empty());
    vzv.zvl_remove(0);
    assert_eq!(vzv.len(), 0);
    assert!(vzv.as_bytes().is_empty());
    vzv.zvl_insert(0, "something");
    assert_eq!(vzv.len(), 1);
    assert!(!vzv.as_bytes().is_empty());
}

#[test]
fn weird_empty_representation_equality() {
    assert_eq!(
        VarZeroVec::<str>::parse_bytes(&[0, 0, 0, 0]).unwrap(),
        VarZeroVec::<str>::parse_bytes(&[]).unwrap()
    );
}
