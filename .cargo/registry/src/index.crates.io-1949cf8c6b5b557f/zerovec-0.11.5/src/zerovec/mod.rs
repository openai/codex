// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "databake")]
mod databake;

#[cfg(feature = "serde")]
mod serde;

mod slice;

pub use slice::ZeroSlice;
pub use slice::ZeroSliceIter;

use crate::ule::*;
#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::cmp::{Ord, Ordering, PartialOrd};
use core::fmt;
#[cfg(feature = "alloc")]
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::num::NonZeroUsize;
use core::ops::Deref;
use core::ptr::NonNull;

/// A zero-copy, byte-aligned vector for fixed-width types.
///
/// `ZeroVec<T>` is designed as a drop-in replacement for `Vec<T>` in situations where it is
/// desirable to borrow data from an unaligned byte slice, such as zero-copy deserialization.
///
/// `T` must implement [`AsULE`], which is auto-implemented for a number of built-in types,
/// including all fixed-width multibyte integers. For variable-width types like [`str`],
/// see [`VarZeroVec`](crate::VarZeroVec). [`zerovec::make_ule`](crate::make_ule) may
/// be used to automatically implement [`AsULE`] for a type and generate the underlying [`ULE`] type.
///
/// Typically, the zero-copy equivalent of a `Vec<T>` will simply be `ZeroVec<'a, T>`.
///
/// Most of the methods on `ZeroVec<'a, T>` come from its [`Deref`] implementation to [`ZeroSlice<T>`](ZeroSlice).
///
/// For creating zero-copy vectors of fixed-size types, see [`VarZeroVec`](crate::VarZeroVec).
///
/// `ZeroVec<T>` behaves much like [`Cow`](alloc::borrow::Cow), where it can be constructed from
/// owned data (and then mutated!) but can also borrow from some buffer.
///
/// # Example
///
/// ```
/// use zerovec::ZeroVec;
///
/// // The little-endian bytes correspond to the numbers on the following line.
/// let nums: &[u16] = &[211, 281, 421, 461];
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Data<'a> {
///     #[serde(borrow)]
///     nums: ZeroVec<'a, u16>,
/// }
///
/// // The owned version will allocate
/// let data = Data {
///     nums: ZeroVec::alloc_from_slice(nums),
/// };
/// let bincode_bytes =
///     bincode::serialize(&data).expect("Serialization should be successful");
///
/// // Will deserialize without allocations
/// let deserialized: Data = bincode::deserialize(&bincode_bytes)
///     .expect("Deserialization should be successful");
///
/// // This deserializes without allocation!
/// assert!(!deserialized.nums.is_owned());
/// assert_eq!(deserialized.nums.get(2), Some(421));
/// assert_eq!(deserialized.nums, nums);
/// ```
///
/// [`ule`]: crate::ule
///
/// # How it Works
///
/// `ZeroVec<T>` represents a slice of `T` as a slice of `T::ULE`. The difference between `T` and
/// `T::ULE` is that `T::ULE` must be encoded in little-endian with 1-byte alignment. When accessing
/// items from `ZeroVec<T>`, we fetch the `T::ULE`, convert it on the fly to `T`, and return `T` by
/// value.
///
/// Benchmarks can be found in the project repository, with some results found in the [crate-level documentation](crate).
///
/// See [the design doc](https://github.com/unicode-org/icu4x/blob/main/utils/zerovec/design_doc.md) for more details.
pub struct ZeroVec<'a, T>
where
    T: AsULE,
{
    vector: EyepatchHackVector<T::ULE>,

    /// Marker type, signalling variance and dropck behavior
    /// by containing all potential types this type represents
    marker1: PhantomData<T::ULE>,
    marker2: PhantomData<&'a T::ULE>,
}

// Send inherits as long as all fields are Send, but also references are Send only
// when their contents are Sync (this is the core purpose of Sync), so
// we need a Send+Sync bound since this struct can logically be a vector or a slice.
unsafe impl<'a, T: AsULE> Send for ZeroVec<'a, T> where T::ULE: Send + Sync {}
// Sync typically inherits as long as all fields are Sync
unsafe impl<'a, T: AsULE> Sync for ZeroVec<'a, T> where T::ULE: Sync {}

impl<'a, T: AsULE> Deref for ZeroVec<'a, T> {
    type Target = ZeroSlice<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

// Represents an unsafe potentially-owned vector/slice type, without a lifetime
// working around dropck limitations.
//
// Must either be constructed by deconstructing a Vec<U>, or from &[U] with capacity set to
// zero. Should not outlive its source &[U] in the borrowed case; this type does not in
// and of itself uphold this guarantee, but the .as_slice() method assumes it.
//
// After https://github.com/rust-lang/rust/issues/34761 stabilizes,
// we should remove this type and use #[may_dangle]
struct EyepatchHackVector<U> {
    /// Pointer to data
    /// This pointer is *always* valid, the reason it is represented as a raw pointer
    /// is that it may logically represent an `&[T::ULE]` or the ptr,len of a `Vec<T::ULE>`
    buf: NonNull<[U]>,
    #[cfg(feature = "alloc")]
    /// Borrowed if zero. Capacity of buffer above if not
    capacity: usize,
}

impl<U> EyepatchHackVector<U> {
    // Return a slice to the inner data for an arbitrary caller-specified lifetime
    #[inline]
    unsafe fn as_arbitrary_slice<'a>(&self) -> &'a [U] {
        self.buf.as_ref()
    }
    // Return a slice to the inner data
    #[inline]
    const fn as_slice<'a>(&'a self) -> &'a [U] {
        // Note: self.buf.as_ref() is not const until 1.73
        unsafe { &*(self.buf.as_ptr() as *const [U]) }
    }

    /// Return this type as a vector
    ///
    /// Data MUST be known to be owned beforehand
    ///
    /// Because this borrows self, this is effectively creating two owners to the same
    /// data, make sure that `self` is cleaned up after this
    ///
    /// (this does not simply take `self` since then it wouldn't be usable from the Drop impl)
    #[cfg(feature = "alloc")]
    unsafe fn get_vec(&self) -> Vec<U> {
        debug_assert!(self.capacity != 0);
        let slice: &[U] = self.as_slice();
        let len = slice.len();
        // Safety: we are assuming owned, and in owned cases
        // this always represents a valid vector
        Vec::from_raw_parts(self.buf.as_ptr() as *mut U, len, self.capacity)
    }

    fn truncate(&mut self, max: usize) {
        // SAFETY:
        // - The elements in buf are `ULE`, so they don't need to be dropped even if we own them.
        // - self.buf is a valid, nonnull slice pointer, since it comes from a NonNull and the struct
        //   invariant requires validity.
        // - Because of the `min`, we are guaranteed to be constructing a slice of the same length or
        //   smaller, from the same pointer, so it will be valid as well, and similarly non-null.
        self.buf = unsafe {
            NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                self.buf.as_mut().as_mut_ptr(),
                core::cmp::min(max, self.buf.as_ref().len()),
            ))
        };
    }
}

#[cfg(feature = "alloc")]
impl<U> Drop for EyepatchHackVector<U> {
    #[inline]
    fn drop(&mut self) {
        if self.capacity != 0 {
            unsafe {
                // we don't need to clean up self here since we're already in a Drop impl
                let _ = self.get_vec();
            }
        }
    }
}

impl<'a, T: AsULE> Clone for ZeroVec<'a, T> {
    fn clone(&self) -> Self {
        #[cfg(feature = "alloc")]
        if self.is_owned() {
            return ZeroVec::new_owned(self.as_ule_slice().into());
        }
        Self {
            vector: EyepatchHackVector {
                buf: self.vector.buf,
                #[cfg(feature = "alloc")]
                capacity: 0,
            },
            marker1: PhantomData,
            marker2: PhantomData,
        }
    }
}

impl<'a, T: AsULE> AsRef<ZeroSlice<T>> for ZeroVec<'a, T> {
    fn as_ref(&self) -> &ZeroSlice<T> {
        self.as_slice()
    }
}

impl<T> fmt::Debug for ZeroVec<'_, T>
where
    T: AsULE + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ZeroVec([")?;
        let mut first = true;
        for el in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{el:?}")?;
            first = false;
        }
        write!(f, "])")
    }
}

impl<T> Eq for ZeroVec<'_, T> where T: AsULE + Eq {}

impl<'a, 'b, T> PartialEq<ZeroVec<'b, T>> for ZeroVec<'a, T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &ZeroVec<'b, T>) -> bool {
        // Note: T implements PartialEq but not T::ULE
        self.iter().eq(other.iter())
    }
}

impl<T> PartialEq<&[T]> for ZeroVec<'_, T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &&[T]) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

impl<T, const N: usize> PartialEq<[T; N]> for ZeroVec<'_, T>
where
    T: AsULE + PartialEq,
{
    #[inline]
    fn eq(&self, other: &[T; N]) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

impl<'a, T: AsULE> Default for ZeroVec<'a, T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T: AsULE + PartialOrd> PartialOrd for ZeroVec<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<'a, T: AsULE + Ord> Ord for ZeroVec<'a, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<'a, T: AsULE> AsRef<[T::ULE]> for ZeroVec<'a, T> {
    fn as_ref(&self) -> &[T::ULE] {
        self.as_ule_slice()
    }
}

impl<'a, T: AsULE> From<&'a [T::ULE]> for ZeroVec<'a, T> {
    fn from(other: &'a [T::ULE]) -> Self {
        ZeroVec::new_borrowed(other)
    }
}

#[cfg(feature = "alloc")]
impl<'a, T: AsULE> From<Vec<T::ULE>> for ZeroVec<'a, T> {
    fn from(other: Vec<T::ULE>) -> Self {
        ZeroVec::new_owned(other)
    }
}

impl<'a, T: AsULE> ZeroVec<'a, T> {
    /// Creates a new, borrowed, empty `ZeroVec<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let zv: ZeroVec<u16> = ZeroVec::new();
    /// assert!(zv.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self::new_borrowed(&[])
    }

    /// Same as `ZeroSlice::len`, which is available through `Deref` and not `const`.
    pub const fn const_len(&self) -> usize {
        self.vector.as_slice().len()
    }

    /// Creates a new owned `ZeroVec` using an existing
    /// allocated backing buffer
    ///
    /// If you have a slice of `&[T]`s, prefer using
    /// [`Self::alloc_from_slice()`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_owned(vec: Vec<T::ULE>) -> Self {
        // Deconstruct the vector into parts
        // This is the only part of the code that goes from Vec
        // to ZeroVec, all other such operations should use this function
        let capacity = vec.capacity();
        let len = vec.len();
        let ptr = core::mem::ManuallyDrop::new(vec).as_mut_ptr();
        // Safety: `ptr` comes from Vec::as_mut_ptr, which says:
        // "Returns an unsafe mutable pointer to the vector’s buffer,
        // or a dangling raw pointer valid for zero sized reads"
        let ptr = unsafe { NonNull::new_unchecked(ptr) };
        let buf = NonNull::slice_from_raw_parts(ptr, len);
        Self {
            vector: EyepatchHackVector { buf, capacity },
            marker1: PhantomData,
            marker2: PhantomData,
        }
    }

    /// Creates a new borrowed `ZeroVec` using an existing
    /// backing buffer
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    pub const fn new_borrowed(slice: &'a [T::ULE]) -> Self {
        // Safety: references in Rust cannot be null.
        // The safe function `impl From<&T> for NonNull<T>` is not const.
        let slice = unsafe { NonNull::new_unchecked(slice as *const [_] as *mut [_]) };
        Self {
            vector: EyepatchHackVector {
                buf: slice,
                #[cfg(feature = "alloc")]
                capacity: 0,
            },
            marker1: PhantomData,
            marker2: PhantomData,
        }
    }

    /// Creates a new, owned, empty `ZeroVec<T>`, with a certain capacity pre-allocated.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::new_owned(Vec::with_capacity(capacity))
    }

    /// Parses a `&[u8]` buffer into a `ZeroVec<T>`.
    ///
    /// This function is infallible for built-in integer types, but fallible for other types,
    /// such as `char`. For more information, see [`ULE::parse_bytes_to_slice`].
    ///
    /// The bytes within the byte buffer must remain constant for the life of the ZeroVec.
    ///
    /// # Endianness
    ///
    /// The byte buffer must be encoded in little-endian, even if running in a big-endian
    /// environment. This ensures a consistent representation of data across platforms.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// assert!(!zerovec.is_owned());
    /// assert_eq!(zerovec.get(2), Some(421));
    /// ```
    pub fn parse_bytes(bytes: &'a [u8]) -> Result<Self, UleError> {
        let slice: &'a [T::ULE] = T::ULE::parse_bytes_to_slice(bytes)?;
        Ok(Self::new_borrowed(slice))
    }

    /// Uses a `&[u8]` buffer as a `ZeroVec<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`ZeroSlice::as_bytes()`].
    pub const unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        // &[u8] and &[T::ULE] are the same slice with different length metadata.
        Self::new_borrowed(core::slice::from_raw_parts(
            bytes.as_ptr() as *const T::ULE,
            bytes.len() / core::mem::size_of::<T::ULE>(),
        ))
    }

    /// Converts a `ZeroVec<T>` into a `ZeroVec<u8>`, retaining the current ownership model.
    ///
    /// Note that the length of the ZeroVec may change.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// Convert a borrowed `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// let zv_bytes = zerovec.into_bytes();
    ///
    /// assert!(!zv_bytes.is_owned());
    /// assert_eq!(zv_bytes.get(0), Some(0xD3));
    /// ```
    ///
    /// Convert an owned `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let nums: &[u16] = &[211, 281, 421, 461];
    /// let zerovec = ZeroVec::alloc_from_slice(nums);
    /// let zv_bytes = zerovec.into_bytes();
    ///
    /// assert!(zv_bytes.is_owned());
    /// assert_eq!(zv_bytes.get(0), Some(0xD3));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn into_bytes(self) -> ZeroVec<'a, u8> {
        use alloc::borrow::Cow;
        match self.into_cow() {
            Cow::Borrowed(slice) => {
                let bytes: &'a [u8] = T::ULE::slice_as_bytes(slice);
                ZeroVec::new_borrowed(bytes)
            }
            Cow::Owned(vec) => {
                let bytes = Vec::from(T::ULE::slice_as_bytes(&vec));
                ZeroVec::new_owned(bytes)
            }
        }
    }

    /// Returns this [`ZeroVec`] as a [`ZeroSlice`].
    ///
    /// To get a reference with a longer lifetime from a borrowed [`ZeroVec`],
    /// use [`ZeroVec::as_maybe_borrowed`].
    #[inline]
    pub const fn as_slice(&self) -> &ZeroSlice<T> {
        let slice: &[T::ULE] = self.vector.as_slice();
        ZeroSlice::from_ule_slice(slice)
    }

    /// Casts a `ZeroVec<T>` to a compatible `ZeroVec<P>`.
    ///
    /// `T` and `P` are compatible if they have the same `ULE` representation.
    ///
    /// If the `ULE`s of `T` and `P` are different types but have the same size,
    /// use [`Self::try_into_converted()`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x80];
    ///
    /// let zerovec_u16: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert_eq!(zerovec_u16.get(3), Some(32973));
    ///
    /// let zerovec_i16: ZeroVec<i16> = zerovec_u16.cast();
    /// assert_eq!(zerovec_i16.get(3), Some(-32563));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn cast<P>(self) -> ZeroVec<'a, P>
    where
        P: AsULE<ULE = T::ULE>,
    {
        match self.into_cow() {
            Cow::Owned(v) => ZeroVec::new_owned(v),
            Cow::Borrowed(v) => ZeroVec::new_borrowed(v),
        }
    }

    /// Converts a `ZeroVec<T>` into a `ZeroVec<P>`, retaining the current ownership model.
    ///
    /// If `T` and `P` have the exact same `ULE`, use [`Self::cast()`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Panics
    ///
    /// Panics if `T::ULE` and `P::ULE` are not the same size.
    ///
    /// # Examples
    ///
    /// Convert a borrowed `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0x7F, 0xF3, 0x01, 0x49, 0xF6, 0x01];
    /// let zv_char: ZeroVec<char> =
    ///     ZeroVec::parse_bytes(bytes).expect("valid code points");
    /// let zv_u8_3: ZeroVec<[u8; 3]> =
    ///     zv_char.try_into_converted().expect("infallible conversion");
    ///
    /// assert!(!zv_u8_3.is_owned());
    /// assert_eq!(zv_u8_3.get(0), Some([0x7F, 0xF3, 0x01]));
    /// ```
    ///
    /// Convert an owned `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let chars: &[char] = &['🍿', '🙉'];
    /// let zv_char = ZeroVec::alloc_from_slice(chars);
    /// let zv_u8_3: ZeroVec<[u8; 3]> =
    ///     zv_char.try_into_converted().expect("length is divisible");
    ///
    /// assert!(zv_u8_3.is_owned());
    /// assert_eq!(zv_u8_3.get(0), Some([0x7F, 0xF3, 0x01]));
    /// ```
    ///
    /// If the types are not the same size, we refuse to convert:
    ///
    /// ```should_panic
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0x7F, 0xF3, 0x01, 0x49, 0xF6, 0x01];
    /// let zv_char: ZeroVec<char> =
    ///     ZeroVec::parse_bytes(bytes).expect("valid code points");
    ///
    /// // Panics! core::mem::size_of::<char::ULE> != core::mem::size_of::<u16::ULE>
    /// zv_char.try_into_converted::<u16>();
    /// ```
    ///
    /// Instead, convert to bytes and then parse:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0x7F, 0xF3, 0x01, 0x49, 0xF6, 0x01];
    /// let zv_char: ZeroVec<char> =
    ///     ZeroVec::parse_bytes(bytes).expect("valid code points");
    /// let zv_u16: ZeroVec<u16> =
    ///     zv_char.into_bytes().try_into_parsed().expect("infallible");
    ///
    /// assert!(!zv_u16.is_owned());
    /// assert_eq!(zv_u16.get(0), Some(0xF37F));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_into_converted<P: AsULE>(self) -> Result<ZeroVec<'a, P>, UleError> {
        assert_eq!(
            core::mem::size_of::<<T as AsULE>::ULE>(),
            core::mem::size_of::<<P as AsULE>::ULE>()
        );
        match self.into_cow() {
            Cow::Borrowed(old_slice) => {
                let bytes: &'a [u8] = T::ULE::slice_as_bytes(old_slice);
                let new_slice = P::ULE::parse_bytes_to_slice(bytes)?;
                Ok(ZeroVec::new_borrowed(new_slice))
            }
            Cow::Owned(old_vec) => {
                let bytes: &[u8] = T::ULE::slice_as_bytes(&old_vec);
                P::ULE::validate_bytes(bytes)?;
                // Feature "vec_into_raw_parts" is not yet stable (#65816). Polyfill:
                let (ptr, len, cap) = {
                    // Take ownership of the pointer
                    let mut v = core::mem::ManuallyDrop::new(old_vec);
                    // Fetch the pointer, length, and capacity
                    (v.as_mut_ptr(), v.len(), v.capacity())
                };
                // Safety checklist for Vec::from_raw_parts:
                // 1. ptr came from a Vec<T>
                // 2. P and T are asserted above to be the same size
                // 3. length is what it was before
                // 4. capacity is what it was before
                let new_vec = unsafe {
                    let ptr = ptr as *mut P::ULE;
                    Vec::from_raw_parts(ptr, len, cap)
                };
                Ok(ZeroVec::new_owned(new_vec))
            }
        }
    }

    /// Check if this type is fully owned
    #[inline]
    pub fn is_owned(&self) -> bool {
        #[cfg(feature = "alloc")]
        return self.vector.capacity != 0;
        #[cfg(not(feature = "alloc"))]
        return false;
    }

    /// If this is a borrowed [`ZeroVec`], return it as a slice that covers
    /// its lifetime parameter.
    ///
    /// To infallibly get a [`ZeroSlice`] with a shorter lifetime, use
    /// [`ZeroVec::as_slice`].
    #[inline]
    pub fn as_maybe_borrowed(&self) -> Option<&'a ZeroSlice<T>> {
        if self.is_owned() {
            None
        } else {
            // We can extend the lifetime of the slice to 'a
            // since we know it is borrowed
            let ule_slice = unsafe { self.vector.as_arbitrary_slice() };
            Some(ZeroSlice::from_ule_slice(ule_slice))
        }
    }

    /// If the ZeroVec is owned, returns the capacity of the vector.
    ///
    /// Otherwise, if the ZeroVec is borrowed, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let mut zv = ZeroVec::<u8>::new_borrowed(&[0, 1, 2, 3]);
    /// assert!(!zv.is_owned());
    /// assert_eq!(zv.owned_capacity(), None);
    ///
    /// // Convert to owned without appending anything
    /// zv.with_mut(|v| ());
    /// assert!(zv.is_owned());
    /// assert_eq!(zv.owned_capacity(), Some(4.try_into().unwrap()));
    ///
    /// // Double the size by appending
    /// zv.with_mut(|v| v.push(0));
    /// assert!(zv.is_owned());
    /// assert_eq!(zv.owned_capacity(), Some(8.try_into().unwrap()));
    /// ```
    #[inline]
    pub fn owned_capacity(&self) -> Option<NonZeroUsize> {
        #[cfg(feature = "alloc")]
        return NonZeroUsize::try_from(self.vector.capacity).ok();
        #[cfg(not(feature = "alloc"))]
        return None;
    }
}

impl<'a> ZeroVec<'a, u8> {
    /// Converts a `ZeroVec<u8>` into a `ZeroVec<T>`, retaining the current ownership model.
    ///
    /// Note that the length of the ZeroVec may change.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// Convert a borrowed `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let zv_bytes = ZeroVec::new_borrowed(bytes);
    /// let zerovec: ZeroVec<u16> = zv_bytes.try_into_parsed().expect("infallible");
    ///
    /// assert!(!zerovec.is_owned());
    /// assert_eq!(zerovec.get(0), Some(211));
    /// ```
    ///
    /// Convert an owned `ZeroVec`:
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: Vec<u8> = vec![0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let zv_bytes = ZeroVec::new_owned(bytes);
    /// let zerovec: ZeroVec<u16> = zv_bytes.try_into_parsed().expect("infallible");
    ///
    /// assert!(zerovec.is_owned());
    /// assert_eq!(zerovec.get(0), Some(211));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_into_parsed<T: AsULE>(self) -> Result<ZeroVec<'a, T>, UleError> {
        match self.into_cow() {
            Cow::Borrowed(bytes) => {
                let slice: &'a [T::ULE] = T::ULE::parse_bytes_to_slice(bytes)?;
                Ok(ZeroVec::new_borrowed(slice))
            }
            Cow::Owned(vec) => {
                let slice = Vec::from(T::ULE::parse_bytes_to_slice(&vec)?);
                Ok(ZeroVec::new_owned(slice))
            }
        }
    }
}

impl<'a, T> ZeroVec<'a, T>
where
    T: AsULE,
{
    /// Creates a `ZeroVec<T>` from a `&[T]` by allocating memory.
    ///
    /// This function results in an `Owned` instance of `ZeroVec<T>`.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// // The little-endian bytes correspond to the numbers on the following line.
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let nums: &[u16] = &[211, 281, 421, 461];
    ///
    /// let zerovec = ZeroVec::alloc_from_slice(nums);
    ///
    /// assert!(zerovec.is_owned());
    /// assert_eq!(bytes, zerovec.as_bytes());
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn alloc_from_slice(other: &[T]) -> Self {
        Self::new_owned(other.iter().copied().map(T::to_unaligned).collect())
    }

    /// Creates a `Vec<T>` from a `ZeroVec<T>`.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let nums: &[u16] = &[211, 281, 421, 461];
    /// let vec: Vec<u16> = ZeroVec::alloc_from_slice(nums).to_vec();
    ///
    /// assert_eq!(nums, vec.as_slice());
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().collect()
    }
}

impl<'a, T> ZeroVec<'a, T>
where
    T: EqULE,
{
    /// Attempts to create a `ZeroVec<'a, T>` from a `&'a [T]` by borrowing the argument.
    ///
    /// If this is not possible, such as on a big-endian platform, `None` is returned.
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// // The little-endian bytes correspond to the numbers on the following line.
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let nums: &[u16] = &[211, 281, 421, 461];
    ///
    /// if let Some(zerovec) = ZeroVec::try_from_slice(nums) {
    ///     assert!(!zerovec.is_owned());
    ///     assert_eq!(bytes, zerovec.as_bytes());
    /// }
    /// ```
    #[inline]
    pub fn try_from_slice(slice: &'a [T]) -> Option<Self> {
        T::slice_to_unaligned(slice).map(|ule_slice| Self::new_borrowed(ule_slice))
    }

    /// Creates a `ZeroVec<'a, T>` from a `&'a [T]`, either by borrowing the argument or by
    /// allocating a new vector.
    ///
    /// This is a cheap operation on little-endian platforms, falling back to a more expensive
    /// operation on big-endian platforms.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// // The little-endian bytes correspond to the numbers on the following line.
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let nums: &[u16] = &[211, 281, 421, 461];
    ///
    /// let zerovec = ZeroVec::from_slice_or_alloc(nums);
    ///
    /// // Note: zerovec could be either borrowed or owned.
    /// assert_eq!(bytes, zerovec.as_bytes());
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn from_slice_or_alloc(slice: &'a [T]) -> Self {
        Self::try_from_slice(slice).unwrap_or_else(|| Self::alloc_from_slice(slice))
    }
}

impl<'a, T> ZeroVec<'a, T>
where
    T: AsULE,
{
    /// Mutates each element according to a given function, meant to be
    /// a more convenient version of calling `.iter_mut()` with
    /// [`ZeroVec::with_mut()`] which serves fewer use cases.
    ///
    /// This will convert the ZeroVec into an owned ZeroVec if not already the case.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// zerovec.for_each_mut(|item| *item += 1);
    ///
    /// assert_eq!(zerovec.to_vec(), &[212, 282, 422, 462]);
    /// assert!(zerovec.is_owned());
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut T)) {
        self.to_mut_slice().iter_mut().for_each(|item| {
            let mut aligned = T::from_unaligned(*item);
            f(&mut aligned);
            *item = aligned.to_unaligned()
        })
    }

    /// Same as [`ZeroVec::for_each_mut()`], but bubbles up errors.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    ///
    /// zerovec.try_for_each_mut(|item| {
    ///     *item = item.checked_add(1).ok_or(())?;
    ///     Ok(())
    /// })?;
    ///
    /// assert_eq!(zerovec.to_vec(), &[212, 282, 422, 462]);
    /// assert!(zerovec.is_owned());
    /// # Ok::<(), ()>(())
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_for_each_mut<E>(
        &mut self,
        mut f: impl FnMut(&mut T) -> Result<(), E>,
    ) -> Result<(), E> {
        self.to_mut_slice().iter_mut().try_for_each(|item| {
            let mut aligned = T::from_unaligned(*item);
            f(&mut aligned)?;
            *item = aligned.to_unaligned();
            Ok(())
        })
    }

    /// Converts a borrowed ZeroVec to an owned ZeroVec. No-op if already owned.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_owned());
    ///
    /// let owned = zerovec.into_owned();
    /// assert!(owned.is_owned());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> ZeroVec<'static, T> {
        use alloc::borrow::Cow;
        match self.into_cow() {
            Cow::Owned(vec) => ZeroVec::new_owned(vec),
            Cow::Borrowed(b) => ZeroVec::new_owned(b.into()),
        }
    }

    /// Allows the ZeroVec to be mutated by converting it to an owned variant, and producing
    /// a mutable vector of ULEs. If you only need a mutable slice, consider using [`Self::to_mut_slice()`]
    /// instead.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::zerovec::ule::AsULE;
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_owned());
    ///
    /// zerovec.with_mut(|v| v.push(12_u16.to_unaligned()));
    /// assert!(zerovec.is_owned());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut alloc::vec::Vec<T::ULE>) -> R) -> R {
        use alloc::borrow::Cow;
        // We're in danger if f() panics whilst we've moved a vector out of self;
        // replace it with an empty dummy vector for now
        let this = core::mem::take(self);
        let mut vec = match this.into_cow() {
            Cow::Owned(v) => v,
            Cow::Borrowed(s) => s.into(),
        };
        let ret = f(&mut vec);
        *self = Self::new_owned(vec);
        ret
    }

    /// Allows the ZeroVec to be mutated by converting it to an owned variant (if necessary)
    /// and returning a slice to its backing buffer. [`Self::with_mut()`] allows for mutation
    /// of the vector itself.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::zerovec::ule::AsULE;
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_owned());
    ///
    /// zerovec.to_mut_slice()[1] = 5u16.to_unaligned();
    /// assert!(zerovec.is_owned());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn to_mut_slice(&mut self) -> &mut [T::ULE] {
        if !self.is_owned() {
            // `buf` is either a valid vector or slice of `T::ULE`s, either
            // way it's always valid
            let slice = self.vector.as_slice();
            *self = ZeroVec::new_owned(slice.into());
        }
        unsafe { self.vector.buf.as_mut() }
    }
    /// Remove all elements from this ZeroVec and reset it to an empty borrowed state.
    pub fn clear(&mut self) {
        *self = Self::new_borrowed(&[])
    }

    /// Removes the first element of the ZeroVec. The ZeroVec remains in the same
    /// borrowed or owned state.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::zerovec::ule::AsULE;
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_owned());
    ///
    /// let first = zerovec.take_first().unwrap();
    /// assert_eq!(first, 0x00D3);
    /// assert!(!zerovec.is_owned());
    ///
    /// let mut zerovec = zerovec.into_owned();
    /// assert!(zerovec.is_owned());
    /// let first = zerovec.take_first().unwrap();
    /// assert_eq!(first, 0x0119);
    /// assert!(zerovec.is_owned());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn take_first(&mut self) -> Option<T> {
        match core::mem::take(self).into_cow() {
            Cow::Owned(mut vec) => {
                if vec.is_empty() {
                    return None;
                }
                let ule = vec.remove(0);
                let rv = T::from_unaligned(ule);
                *self = ZeroVec::new_owned(vec);
                Some(rv)
            }
            Cow::Borrowed(b) => {
                let (ule, remainder) = b.split_first()?;
                let rv = T::from_unaligned(*ule);
                *self = ZeroVec::new_borrowed(remainder);
                Some(rv)
            }
        }
    }

    /// Removes the last element of the ZeroVec. The ZeroVec remains in the same
    /// borrowed or owned state.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::zerovec::ule::AsULE;
    /// use zerovec::ZeroVec;
    ///
    /// let bytes: &[u8] = &[0xD3, 0x00, 0x19, 0x01, 0xA5, 0x01, 0xCD, 0x01];
    /// let mut zerovec: ZeroVec<u16> =
    ///     ZeroVec::parse_bytes(bytes).expect("infallible");
    /// assert!(!zerovec.is_owned());
    ///
    /// let last = zerovec.take_last().unwrap();
    /// assert_eq!(last, 0x01CD);
    /// assert!(!zerovec.is_owned());
    ///
    /// let mut zerovec = zerovec.into_owned();
    /// assert!(zerovec.is_owned());
    /// let last = zerovec.take_last().unwrap();
    /// assert_eq!(last, 0x01A5);
    /// assert!(zerovec.is_owned());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn take_last(&mut self) -> Option<T> {
        match core::mem::take(self).into_cow() {
            Cow::Owned(mut vec) => {
                let ule = vec.pop()?;
                let rv = T::from_unaligned(ule);
                *self = ZeroVec::new_owned(vec);
                Some(rv)
            }
            Cow::Borrowed(b) => {
                let (ule, remainder) = b.split_last()?;
                let rv = T::from_unaligned(*ule);
                *self = ZeroVec::new_borrowed(remainder);
                Some(rv)
            }
        }
    }

    /// Converts the type into a `Cow<'a, [T::ULE]>`, which is
    /// the logical equivalent of this type's internal representation
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn into_cow(self) -> Cow<'a, [T::ULE]> {
        let this = core::mem::ManuallyDrop::new(self);
        if this.is_owned() {
            let vec = unsafe {
                // safe to call: we know it's owned,
                // and `self`/`this` are thenceforth no longer used or dropped
                { this }.vector.get_vec()
            };
            Cow::Owned(vec)
        } else {
            // We can extend the lifetime of the slice to 'a
            // since we know it is borrowed
            let slice = unsafe { { this }.vector.as_arbitrary_slice() };
            Cow::Borrowed(slice)
        }
    }

    /// Truncates this vector to `min(self.len(), max)`.
    #[inline]
    pub fn truncated(mut self, max: usize) -> Self {
        self.vector.truncate(max);
        self
    }
}

#[cfg(feature = "alloc")]
impl<T: AsULE> FromIterator<T> for ZeroVec<'_, T> {
    /// Creates an owned [`ZeroVec`] from an iterator of values.
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        ZeroVec::new_owned(iter.into_iter().map(|t| t.to_unaligned()).collect())
    }
}

/// Convenience wrapper for [`ZeroSlice::from_ule_slice`]. The value will be created at compile-time,
/// meaning that all arguments must also be constant.
///
/// # Arguments
///
/// * `$aligned` - The type of an element in its canonical, aligned form, e.g., `char`.
/// * `$convert` - A const function that converts an `$aligned` into its unaligned equivalent, e.g.,
///   const fn from_aligned(a: CanonicalType) -> CanonicalType::ULE`.
/// * `$x` - The elements that the `ZeroSlice` will hold.
///
/// # Examples
///
/// Using array-conversion functions provided by this crate:
///
/// ```
/// use zerovec::{ZeroSlice, zeroslice, ule::AsULE};
///
/// const SIGNATURE: &ZeroSlice<char> = zeroslice!(char; <char as AsULE>::ULE::from_aligned; ['b', 'y', 'e', '✌']);
/// const EMPTY: &ZeroSlice<u32> = zeroslice![];
///
/// let empty: &ZeroSlice<u32> = zeroslice![];
/// let nums = zeroslice!(u32; <u32 as AsULE>::ULE::from_unsigned; [1, 2, 3, 4, 5]);
/// assert_eq!(nums.last().unwrap(), 5);
/// ```
///
/// Using a custom array-conversion function:
///
/// ```
/// use zerovec::{ule::AsULE, ule::RawBytesULE, zeroslice, ZeroSlice};
///
/// const fn be_convert(num: i16) -> <i16 as AsULE>::ULE {
///     RawBytesULE(num.to_be_bytes())
/// }
///
/// const NUMBERS_BE: &ZeroSlice<i16> =
///     zeroslice!(i16; be_convert; [1, -2, 3, -4, 5]);
/// ```
#[macro_export]
macro_rules! zeroslice {
    () => {
        $crate::ZeroSlice::new_empty()
    };
    ($aligned:ty; $convert:expr; [$($x:expr),+ $(,)?]) => {
        $crate::ZeroSlice::<$aligned>::from_ule_slice(const { &[$($convert($x)),*] })
    };
}

/// Creates a borrowed `ZeroVec`. Convenience wrapper for `zeroslice!(...).as_zerovec()`. The value
/// will be created at compile-time, meaning that all arguments must also be constant.
///
/// See [`zeroslice!`](crate::zeroslice) for more information.
///
/// # Examples
///
/// ```
/// use zerovec::{ZeroVec, zerovec, ule::AsULE};
///
/// const SIGNATURE: ZeroVec<char> = zerovec!(char; <char as AsULE>::ULE::from_aligned; ['a', 'y', 'e', '✌']);
/// assert!(!SIGNATURE.is_owned());
///
/// const EMPTY: ZeroVec<u32> = zerovec![];
/// assert!(!EMPTY.is_owned());
/// ```
#[macro_export]
macro_rules! zerovec {
    () => (
        $crate::ZeroVec::new()
    );
    ($aligned:ty; $convert:expr; [$($x:expr),+ $(,)?]) => (
        $crate::zeroslice![$aligned; $convert; [$($x),+]].as_zerovec()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samples::*;

    #[test]
    fn test_get() {
        {
            let zerovec = ZeroVec::from_slice_or_alloc(TEST_SLICE);
            assert_eq!(zerovec.get(0), Some(TEST_SLICE[0]));
            assert_eq!(zerovec.get(1), Some(TEST_SLICE[1]));
            assert_eq!(zerovec.get(2), Some(TEST_SLICE[2]));
        }
        {
            let zerovec = ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap();
            assert_eq!(zerovec.get(0), Some(TEST_SLICE[0]));
            assert_eq!(zerovec.get(1), Some(TEST_SLICE[1]));
            assert_eq!(zerovec.get(2), Some(TEST_SLICE[2]));
        }
    }

    #[test]
    fn test_binary_search() {
        {
            let zerovec = ZeroVec::from_slice_or_alloc(TEST_SLICE);
            assert_eq!(Ok(3), zerovec.binary_search(&0x0e0d0c));
            assert_eq!(Err(3), zerovec.binary_search(&0x0c0d0c));
        }
        {
            let zerovec = ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap();
            assert_eq!(Ok(3), zerovec.binary_search(&0x0e0d0c));
            assert_eq!(Err(3), zerovec.binary_search(&0x0c0d0c));
        }
    }

    #[test]
    fn test_odd_alignment() {
        assert_eq!(
            Some(0x020100),
            ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap().get(0)
        );
        assert_eq!(
            Some(0x04000201),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[1..77])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x05040002),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[2..78])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x06050400),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[3..79])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x060504),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[4..])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x4e4d4c00),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[75..79])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x4e4d4c00),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[3..79])
                .unwrap()
                .get(18)
        );
        assert_eq!(
            Some(0x4e4d4c),
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[76..])
                .unwrap()
                .get(0)
        );
        assert_eq!(
            Some(0x4e4d4c),
            ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap().get(19)
        );
        // TODO(#1144): Check for correct slice length in RawBytesULE
        // assert_eq!(
        //     None,
        //     ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[77..])
        //         .unwrap()
        //         .get(0)
        // );
        assert_eq!(
            None,
            ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap().get(20)
        );
        assert_eq!(
            None,
            ZeroVec::<u32>::parse_bytes(&TEST_BUFFER_LE[3..79])
                .unwrap()
                .get(19)
        );
    }
}
