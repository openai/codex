use alloc::borrow::{Borrow, BorrowMut, Cow, ToOwned};
use alloc::fmt;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::sync::Arc;
#[cfg(feature = "std")]
use core::any::Any;
use core::iter::FromIterator;
use core::mem;
use core::ops::{Add, AddAssign, Deref, DerefMut, Index, IndexMut};
use core::str::FromStr;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(feature = "std")]
use std::ffi::{CStr, CString};

use ascii_char::AsciiChar;
use ascii_str::{AsAsciiStr, AsAsciiStrError, AsciiStr};

/// A growable string stored as an ASCII encoded buffer.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AsciiString {
    vec: Vec<AsciiChar>,
}

impl AsciiString {
    /// Creates a new, empty ASCII string buffer without allocating.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::new();
    /// ```
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        AsciiString { vec: Vec::new() }
    }

    /// Creates a new ASCII string buffer with the given capacity.
    /// The string will be able to hold exactly `capacity` bytes without reallocating.
    /// If `capacity` is 0, the ASCII string will not allocate.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::with_capacity(10);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        AsciiString {
            vec: Vec::with_capacity(capacity),
        }
    }

    /// Creates a new `AsciiString` from a length, capacity and pointer.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't checked:
    ///
    /// * The memory at `buf` need to have been previously allocated by the same allocator this
    ///   library uses, with an alignment of 1.
    /// * `length` needs to be less than or equal to `capacity`.
    /// * `capacity` needs to be the correct value.
    /// * `buf` must have `length` valid ascii elements and contain a total of `capacity` total,
    ///   possibly, uninitialized, elements.
    /// * Nothing else must be using the memory `buf` points to.
    ///
    /// Violating these may cause problems like corrupting the allocator's internal data structures.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use ascii::AsciiString;
    /// use std::mem;
    ///
    /// unsafe {
    ///    let mut s = AsciiString::from_ascii("hello").unwrap();
    ///    let ptr = s.as_mut_ptr();
    ///    let len = s.len();
    ///    let capacity = s.capacity();
    ///
    ///    mem::forget(s);
    ///
    ///    let s = AsciiString::from_raw_parts(ptr, len, capacity);
    ///
    ///    assert_eq!(AsciiString::from_ascii("hello").unwrap(), s);
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub unsafe fn from_raw_parts(buf: *mut AsciiChar, length: usize, capacity: usize) -> Self {
        AsciiString {
            // SAFETY: Caller guarantees that `buf` was previously allocated by this library,
            //         that `buf` contains `length` valid ascii elements and has a total capacity
            //         of `capacity` elements, and that nothing else is using the momory.
            vec: unsafe { Vec::from_raw_parts(buf, length, capacity) },
        }
    }

    /// Converts a vector of bytes to an `AsciiString` without checking for non-ASCII characters.
    ///
    /// # Safety
    /// This function is unsafe because it does not check that the bytes passed to it are valid
    /// ASCII characters. If this constraint is violated, it may cause memory unsafety issues with
    /// future of the `AsciiString`, as the rest of this library assumes that `AsciiString`s are
    /// ASCII encoded.
    #[inline]
    #[must_use]
    pub unsafe fn from_ascii_unchecked<B>(bytes: B) -> Self
    where
        B: Into<Vec<u8>>,
    {
        let mut bytes = bytes.into();
        // SAFETY: The caller guarantees all bytes are valid ascii bytes.
        let ptr = bytes.as_mut_ptr().cast::<AsciiChar>();
        let length = bytes.len();
        let capacity = bytes.capacity();
        mem::forget(bytes);

        // SAFETY: We guarantee all invariants, as we got the
        //         pointer, length and capacity from a `Vec`,
        //         and we also guarantee the pointer is valid per
        //         the `SAFETY` notice above.
        let vec = Vec::from_raw_parts(ptr, length, capacity);

        Self { vec }
    }

    /// Converts anything that can represent a byte buffer into an `AsciiString`.
    ///
    /// # Errors
    /// Returns the byte buffer if not all of the bytes are ASCII characters.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let foo = AsciiString::from_ascii("foo".to_string()).unwrap();
    /// let err = AsciiString::from_ascii("Ŋ".to_string()).unwrap_err();
    /// assert_eq!(foo.as_str(), "foo");
    /// assert_eq!(err.into_source(), "Ŋ");
    /// ```
    pub fn from_ascii<B>(bytes: B) -> Result<AsciiString, FromAsciiError<B>>
    where
        B: Into<Vec<u8>> + AsRef<[u8]>,
    {
        match bytes.as_ref().as_ascii_str() {
            // SAFETY: `as_ascii_str` guarantees all bytes are valid ascii bytes.
            Ok(_) => Ok(unsafe { AsciiString::from_ascii_unchecked(bytes) }),
            Err(e) => Err(FromAsciiError {
                error: e,
                owner: bytes,
            }),
        }
    }

    /// Pushes the given ASCII string onto this ASCII string buffer.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{AsciiString, AsAsciiStr};
    /// use std::str::FromStr;
    /// let mut s = AsciiString::from_str("foo").unwrap();
    /// s.push_str("bar".as_ascii_str().unwrap());
    /// assert_eq!(s, "foobar".as_ascii_str().unwrap());
    /// ```
    #[inline]
    pub fn push_str(&mut self, string: &AsciiStr) {
        self.vec.extend(string.chars());
    }

    /// Inserts the given ASCII string at the given place in this ASCII string buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than the `AsciiString`'s length.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{AsciiString, AsAsciiStr};
    /// use std::str::FromStr;
    /// let mut s = AsciiString::from_str("abc").unwrap();
    /// s.insert_str(1, "def".as_ascii_str().unwrap());
    /// assert_eq!(&*s, "adefbc");
    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &AsciiStr) {
        self.vec.reserve(string.len());
        self.vec.splice(idx..idx, string.into_iter().copied());
    }

    /// Returns the number of bytes that this ASCII string buffer can hold without reallocating.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let s = String::with_capacity(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }

    /// Reserves capacity for at least `additional` more bytes to be inserted in the given
    /// `AsciiString`. The collection may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::new();
    /// s.reserve(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.vec.reserve(additional);
    }

    /// Reserves the minimum capacity for exactly `additional` more bytes to be inserted in the
    /// given `AsciiString`. Does nothing if the capacity is already sufficient.
    ///
    /// Note that the allocator may give the collection more space than it requests. Therefore
    /// capacity can not be relied upon to be precisely minimal. Prefer `reserve` if future
    /// insertions are expected.
    ///
    /// # Panics
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::new();
    /// s.reserve_exact(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]

    pub fn reserve_exact(&mut self, additional: usize) {
        self.vec.reserve_exact(additional);
    }

    /// Shrinks the capacity of this ASCII string buffer to match it's length.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// use std::str::FromStr;
    /// let mut s = AsciiString::from_str("foo").unwrap();
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    /// s.shrink_to_fit();
    /// assert_eq!(s.capacity(), 3);
    /// ```
    #[inline]

    pub fn shrink_to_fit(&mut self) {
        self.vec.shrink_to_fit();
    }

    /// Adds the given ASCII character to the end of the ASCII string.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{ AsciiChar, AsciiString};
    /// let mut s = AsciiString::from_ascii("abc").unwrap();
    /// s.push(AsciiChar::from_ascii('1').unwrap());
    /// s.push(AsciiChar::from_ascii('2').unwrap());
    /// s.push(AsciiChar::from_ascii('3').unwrap());
    /// assert_eq!(s, "abc123");
    /// ```
    #[inline]

    pub fn push(&mut self, ch: AsciiChar) {
        self.vec.push(ch);
    }

    /// Shortens a ASCII string to the specified length.
    ///
    /// # Panics
    /// Panics if `new_len` > current length.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::from_ascii("hello").unwrap();
    /// s.truncate(2);
    /// assert_eq!(s, "he");
    /// ```
    #[inline]

    pub fn truncate(&mut self, new_len: usize) {
        self.vec.truncate(new_len);
    }

    /// Removes the last character from the ASCII string buffer and returns it.
    /// Returns `None` if this string buffer is empty.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::from_ascii("foo").unwrap();
    /// assert_eq!(s.pop().map(|c| c.as_char()), Some('o'));
    /// assert_eq!(s.pop().map(|c| c.as_char()), Some('o'));
    /// assert_eq!(s.pop().map(|c| c.as_char()), Some('f'));
    /// assert_eq!(s.pop(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn pop(&mut self) -> Option<AsciiChar> {
        self.vec.pop()
    }

    /// Removes the ASCII character at position `idx` from the buffer and returns it.
    ///
    /// # Warning
    /// This is an O(n) operation as it requires copying every element in the buffer.
    ///
    /// # Panics
    /// If `idx` is out of bounds this function will panic.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::from_ascii("foo").unwrap();
    /// assert_eq!(s.remove(0).as_char(), 'f');
    /// assert_eq!(s.remove(1).as_char(), 'o');
    /// assert_eq!(s.remove(0).as_char(), 'o');
    /// ```
    #[inline]
    #[must_use]
    pub fn remove(&mut self, idx: usize) -> AsciiChar {
        self.vec.remove(idx)
    }

    /// Inserts an ASCII character into the buffer at position `idx`.
    ///
    /// # Warning
    /// This is an O(n) operation as it requires copying every element in the buffer.
    ///
    /// # Panics
    /// If `idx` is out of bounds this function will panic.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{AsciiString,AsciiChar};
    /// let mut s = AsciiString::from_ascii("foo").unwrap();
    /// s.insert(2, AsciiChar::b);
    /// assert_eq!(s, "fobo");
    /// ```
    #[inline]

    pub fn insert(&mut self, idx: usize, ch: AsciiChar) {
        self.vec.insert(idx, ch);
    }

    /// Returns the number of bytes in this ASCII string.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let s = AsciiString::from_ascii("foo").unwrap();
    /// assert_eq!(s.len(), 3);
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    /// Returns true if the ASCII string contains zero bytes.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{AsciiChar, AsciiString};
    /// let mut s = AsciiString::new();
    /// assert!(s.is_empty());
    /// s.push(AsciiChar::from_ascii('a').unwrap());
    /// assert!(!s.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Truncates the ASCII string, setting length (but not capacity) to zero.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiString;
    /// let mut s = AsciiString::from_ascii("foo").unwrap();
    /// s.clear();
    /// assert!(s.is_empty());
    /// ```
    #[inline]

    pub fn clear(&mut self) {
        self.vec.clear();
    }

    /// Converts this [`AsciiString`] into a [`Box`]`<`[`AsciiStr`]`>`.
    ///
    /// This will drop any excess capacity
    #[inline]
    #[must_use]
    pub fn into_boxed_ascii_str(self) -> Box<AsciiStr> {
        let slice = self.vec.into_boxed_slice();
        Box::from(slice)
    }
}

impl Deref for AsciiString {
    type Target = AsciiStr;

    #[inline]
    fn deref(&self) -> &AsciiStr {
        self.vec.as_slice().as_ref()
    }
}

impl DerefMut for AsciiString {
    #[inline]
    fn deref_mut(&mut self) -> &mut AsciiStr {
        self.vec.as_mut_slice().as_mut()
    }
}

impl PartialEq<str> for AsciiString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        **self == *other
    }
}

impl PartialEq<AsciiString> for str {
    #[inline]
    fn eq(&self, other: &AsciiString) -> bool {
        **other == *self
    }
}

macro_rules! impl_eq {
    ($lhs:ty, $rhs:ty) => {
        impl PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                PartialEq::eq(&**self, &**other)
            }
        }
    };
}

impl_eq! { AsciiString, String }
impl_eq! { String, AsciiString }
impl_eq! { &AsciiStr, String }
impl_eq! { String, &AsciiStr }
impl_eq! { &AsciiStr, AsciiString }
impl_eq! { AsciiString, &AsciiStr }
impl_eq! { &str, AsciiString }
impl_eq! { AsciiString, &str }

impl Borrow<AsciiStr> for AsciiString {
    #[inline]
    fn borrow(&self) -> &AsciiStr {
        &**self
    }
}

impl BorrowMut<AsciiStr> for AsciiString {
    #[inline]
    fn borrow_mut(&mut self) -> &mut AsciiStr {
        &mut **self
    }
}

impl From<Vec<AsciiChar>> for AsciiString {
    #[inline]
    fn from(vec: Vec<AsciiChar>) -> Self {
        AsciiString { vec }
    }
}

impl From<AsciiChar> for AsciiString {
    #[inline]
    fn from(ch: AsciiChar) -> Self {
        AsciiString { vec: vec![ch] }
    }
}

impl From<AsciiString> for Vec<u8> {
    fn from(mut s: AsciiString) -> Vec<u8> {
        // SAFETY: All ascii bytes are valid `u8`, as we are `repr(u8)`.
        // Note: We forget `self` to avoid `self.vec` from being deallocated.
        let ptr = s.vec.as_mut_ptr().cast::<u8>();
        let length = s.vec.len();
        let capacity = s.vec.capacity();
        mem::forget(s);

        // SAFETY: We guarantee all invariants due to getting `ptr`, `length`
        //         and `capacity` from a `Vec`. We also guarantee `ptr` is valid
        //         due to the `SAFETY` block above.
        unsafe { Vec::from_raw_parts(ptr, length, capacity) }
    }
}

impl From<AsciiString> for Vec<AsciiChar> {
    fn from(s: AsciiString) -> Vec<AsciiChar> {
        s.vec
    }
}

impl<'a> From<&'a AsciiStr> for AsciiString {
    #[inline]
    fn from(s: &'a AsciiStr) -> Self {
        s.to_ascii_string()
    }
}

impl<'a> From<&'a [AsciiChar]> for AsciiString {
    #[inline]
    fn from(s: &'a [AsciiChar]) -> AsciiString {
        s.iter().copied().collect()
    }
}

impl From<AsciiString> for String {
    #[inline]
    fn from(s: AsciiString) -> String {
        // SAFETY: All ascii bytes are `utf8`.
        unsafe { String::from_utf8_unchecked(s.into()) }
    }
}

impl From<Box<AsciiStr>> for AsciiString {
    #[inline]
    fn from(boxed: Box<AsciiStr>) -> Self {
        boxed.into_ascii_string()
    }
}

impl From<AsciiString> for Box<AsciiStr> {
    #[inline]
    fn from(string: AsciiString) -> Self {
        string.into_boxed_ascii_str()
    }
}

impl From<AsciiString> for Rc<AsciiStr> {
    fn from(s: AsciiString) -> Rc<AsciiStr> {
        let var: Rc<[AsciiChar]> = s.vec.into();
        // SAFETY: AsciiStr is repr(transparent) and thus has the same layout as [AsciiChar]
        unsafe { Rc::from_raw(Rc::into_raw(var) as *const AsciiStr) }
    }
}

impl From<AsciiString> for Arc<AsciiStr> {
    fn from(s: AsciiString) -> Arc<AsciiStr> {
        let var: Arc<[AsciiChar]> = s.vec.into();
        // SAFETY: AsciiStr is repr(transparent) and thus has the same layout as [AsciiChar]
        unsafe { Arc::from_raw(Arc::into_raw(var) as *const AsciiStr) }
    }
}

impl<'a> From<Cow<'a, AsciiStr>> for AsciiString {
    fn from(cow: Cow<'a, AsciiStr>) -> AsciiString {
        cow.into_owned()
    }
}

impl From<AsciiString> for Cow<'static, AsciiStr> {
    fn from(string: AsciiString) -> Cow<'static, AsciiStr> {
        Cow::Owned(string)
    }
}

impl<'a> From<&'a AsciiStr> for Cow<'a, AsciiStr> {
    fn from(s: &'a AsciiStr) -> Cow<'a, AsciiStr> {
        Cow::Borrowed(s)
    }
}

impl AsRef<AsciiStr> for AsciiString {
    #[inline]
    fn as_ref(&self) -> &AsciiStr {
        &**self
    }
}

impl AsRef<[AsciiChar]> for AsciiString {
    #[inline]
    fn as_ref(&self) -> &[AsciiChar] {
        &self.vec
    }
}

impl AsRef<[u8]> for AsciiString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<str> for AsciiString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsMut<AsciiStr> for AsciiString {
    #[inline]
    fn as_mut(&mut self) -> &mut AsciiStr {
        &mut *self
    }
}

impl AsMut<[AsciiChar]> for AsciiString {
    #[inline]
    fn as_mut(&mut self) -> &mut [AsciiChar] {
        &mut self.vec
    }
}

impl FromStr for AsciiString {
    type Err = AsAsciiStrError;

    fn from_str(s: &str) -> Result<AsciiString, AsAsciiStrError> {
        s.as_ascii_str().map(AsciiStr::to_ascii_string)
    }
}

impl fmt::Display for AsciiString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl fmt::Debug for AsciiString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/// Please note that the `std::fmt::Result` returned by these methods does not support
/// transmission of an error other than that an error occurred.
impl fmt::Write for AsciiString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(astr) = AsciiStr::from_ascii(s) {
            self.push_str(astr);
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        if let Ok(achar) = AsciiChar::from_ascii(c) {
            self.push(achar);
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

impl<A: AsRef<AsciiStr>> FromIterator<A> for AsciiString {
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> AsciiString {
        let mut buf = AsciiString::new();
        buf.extend(iter);
        buf
    }
}

impl<A: AsRef<AsciiStr>> Extend<A> for AsciiString {
    fn extend<I: IntoIterator<Item = A>>(&mut self, iterable: I) {
        let iterator = iterable.into_iter();
        let (lower_bound, _) = iterator.size_hint();
        self.reserve(lower_bound);
        for item in iterator {
            self.push_str(item.as_ref());
        }
    }
}

impl<'a> Add<&'a AsciiStr> for AsciiString {
    type Output = AsciiString;

    #[inline]
    fn add(mut self, other: &AsciiStr) -> AsciiString {
        self.push_str(other);
        self
    }
}

impl<'a> AddAssign<&'a AsciiStr> for AsciiString {
    #[inline]
    fn add_assign(&mut self, other: &AsciiStr) {
        self.push_str(other);
    }
}

#[allow(clippy::indexing_slicing)] // In `Index`, if it's out of bounds, panic is the default
impl<T> Index<T> for AsciiString
where
    AsciiStr: Index<T>,
{
    type Output = <AsciiStr as Index<T>>::Output;

    #[inline]
    fn index(&self, index: T) -> &<AsciiStr as Index<T>>::Output {
        &(**self)[index]
    }
}

#[allow(clippy::indexing_slicing)] // In `IndexMut`, if it's out of bounds, panic is the default
impl<T> IndexMut<T> for AsciiString
where
    AsciiStr: IndexMut<T>,
{
    #[inline]
    fn index_mut(&mut self, index: T) -> &mut <AsciiStr as Index<T>>::Output {
        &mut (**self)[index]
    }
}

/// A possible error value when converting an `AsciiString` from a byte vector or string.
/// It wraps an `AsAsciiStrError` which you can get through the `ascii_error()` method.
///
/// This is the error type for `AsciiString::from_ascii()` and
/// `IntoAsciiString::into_ascii_string()`. They will never clone or touch the content of the
/// original type; It can be extracted by the `into_source` method.
///
/// #Examples
/// ```
/// # use ascii::IntoAsciiString;
/// let err = "bø!".to_string().into_ascii_string().unwrap_err();
/// assert_eq!(err.ascii_error().valid_up_to(), 1);
/// assert_eq!(err.into_source(), "bø!".to_string());
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FromAsciiError<O> {
    error: AsAsciiStrError,
    owner: O,
}
impl<O> FromAsciiError<O> {
    /// Get the position of the first non-ASCII byte or character.
    #[inline]
    #[must_use]
    pub fn ascii_error(&self) -> AsAsciiStrError {
        self.error
    }
    /// Get back the original, unmodified type.
    #[inline]
    #[must_use]
    pub fn into_source(self) -> O {
        self.owner
    }
}
impl<O> fmt::Debug for FromAsciiError<O> {
    #[inline]
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.error, fmtr)
    }
}
impl<O> fmt::Display for FromAsciiError<O> {
    #[inline]
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, fmtr)
    }
}
#[cfg(feature = "std")]
impl<O: Any> Error for FromAsciiError<O> {
    #[inline]
    #[allow(deprecated)] // TODO: Remove deprecation once the earliest version we support deprecates this method.
    fn description(&self) -> &str {
        self.error.description()
    }
    /// Always returns an `AsAsciiStrError`
    fn cause(&self) -> Option<&dyn Error> {
        Some(&self.error as &dyn Error)
    }
}

/// Convert vectors into `AsciiString`.
pub trait IntoAsciiString: Sized {
    /// Convert to `AsciiString` without checking for non-ASCII characters.
    ///
    /// # Safety
    /// If `self` contains non-ascii characters, calling this function is
    /// undefined behavior.
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString;

    /// Convert to `AsciiString`.
    ///
    /// # Errors
    /// If `self` contains non-ascii characters, this will return `Err`
    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>>;
}

impl IntoAsciiString for Vec<AsciiChar> {
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        AsciiString::from(self)
    }
    #[inline]
    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        Ok(AsciiString::from(self))
    }
}

impl<'a> IntoAsciiString for &'a [AsciiChar] {
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        AsciiString::from(self)
    }
    #[inline]
    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        Ok(AsciiString::from(self))
    }
}

impl<'a> IntoAsciiString for &'a AsciiStr {
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        AsciiString::from(self)
    }
    #[inline]
    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        Ok(AsciiString::from(self))
    }
}

macro_rules! impl_into_ascii_string {
    ('a, $wider:ty) => {
        impl<'a> IntoAsciiString for $wider {
            #[inline]
            unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
                // SAFETY: Caller guarantees `self` only has valid ascii bytes
                unsafe { AsciiString::from_ascii_unchecked(self) }
            }

            #[inline]
            fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
                AsciiString::from_ascii(self)
            }
        }
    };

    ($wider:ty) => {
        impl IntoAsciiString for $wider {
            #[inline]
            unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
                // SAFETY: Caller guarantees `self` only has valid ascii bytes
                unsafe { AsciiString::from_ascii_unchecked(self) }
            }

            #[inline]
            fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
                AsciiString::from_ascii(self)
            }
        }
    };
}

impl_into_ascii_string! {AsciiString}
impl_into_ascii_string! {Vec<u8>}
impl_into_ascii_string! {'a, &'a [u8]}
impl_into_ascii_string! {String}
impl_into_ascii_string! {'a, &'a str}

/// # Notes
/// The trailing null byte `CString` has will be removed during this conversion.
#[cfg(feature = "std")]
impl IntoAsciiString for CString {
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        // SAFETY: Caller guarantees `self` only has valid ascii bytes
        unsafe { AsciiString::from_ascii_unchecked(self.into_bytes()) }
    }

    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        AsciiString::from_ascii(self.into_bytes_with_nul())
            .map_err(|FromAsciiError { error, owner }| {
                FromAsciiError {
                    // SAFETY: We don't discard the NULL byte from the original
                    //         string, so we ensure that it's null terminated
                    owner: unsafe { CString::from_vec_unchecked(owner) },
                    error,
                }
            })
            .map(|mut s| {
                let nul = s.pop();
                debug_assert_eq!(nul, Some(AsciiChar::Null));
                s
            })
    }
}

/// Note that the trailing null byte will be removed in the conversion.
#[cfg(feature = "std")]
impl<'a> IntoAsciiString for &'a CStr {
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        // SAFETY: Caller guarantees `self` only has valid ascii bytes
        unsafe { AsciiString::from_ascii_unchecked(self.to_bytes()) }
    }

    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        AsciiString::from_ascii(self.to_bytes_with_nul())
            .map_err(|FromAsciiError { error, owner }| FromAsciiError {
                // SAFETY: We don't discard the NULL byte from the original
                //         string, so we ensure that it's null terminated
                owner: unsafe { CStr::from_ptr(owner.as_ptr().cast()) },
                error,
            })
            .map(|mut s| {
                let nul = s.pop();
                debug_assert_eq!(nul, Some(AsciiChar::Null));
                s
            })
    }
}

impl<'a, B> IntoAsciiString for Cow<'a, B>
where
    B: 'a + ToOwned + ?Sized,
    &'a B: IntoAsciiString,
    <B as ToOwned>::Owned: IntoAsciiString,
{
    #[inline]
    unsafe fn into_ascii_string_unchecked(self) -> AsciiString {
        // SAFETY: Caller guarantees `self` only has valid ascii bytes
        unsafe { IntoAsciiString::into_ascii_string_unchecked(self.into_owned()) }
    }

    fn into_ascii_string(self) -> Result<AsciiString, FromAsciiError<Self>> {
        match self {
            Cow::Owned(b) => {
                IntoAsciiString::into_ascii_string(b).map_err(|FromAsciiError { error, owner }| {
                    FromAsciiError {
                        owner: Cow::Owned(owner),
                        error,
                    }
                })
            }
            Cow::Borrowed(b) => {
                IntoAsciiString::into_ascii_string(b).map_err(|FromAsciiError { error, owner }| {
                    FromAsciiError {
                        owner: Cow::Borrowed(owner),
                        error,
                    }
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AsciiString, IntoAsciiString};
    use alloc::str::FromStr;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use alloc::boxed::Box;
    #[cfg(feature = "std")]
    use std::ffi::CString;
    use {AsciiChar, AsciiStr};

    #[test]
    fn into_string() {
        let v = AsciiString::from_ascii(&[40_u8, 32, 59][..]).unwrap();
        assert_eq!(Into::<String>::into(v), "( ;".to_string());
    }

    #[test]
    fn into_bytes() {
        let v = AsciiString::from_ascii(&[40_u8, 32, 59][..]).unwrap();
        assert_eq!(Into::<Vec<u8>>::into(v), vec![40_u8, 32, 59]);
    }

    #[test]
    fn from_ascii_vec() {
        let vec = vec![
            AsciiChar::from_ascii('A').unwrap(),
            AsciiChar::from_ascii('B').unwrap(),
        ];
        assert_eq!(AsciiString::from(vec), AsciiString::from_str("AB").unwrap());
    }

    #[test]
    #[cfg(feature = "std")]
    fn from_cstring() {
        let cstring = CString::new("baz").unwrap();
        let ascii_str = cstring.clone().into_ascii_string().unwrap();
        let expected_chars = &[AsciiChar::b, AsciiChar::a, AsciiChar::z];
        assert_eq!(ascii_str.len(), 3);
        assert_eq!(ascii_str.as_slice(), expected_chars);

        // SAFETY: "baz" only contains valid ascii characters.
        let ascii_str_unchecked = unsafe { cstring.into_ascii_string_unchecked() };
        assert_eq!(ascii_str_unchecked.len(), 3);
        assert_eq!(ascii_str_unchecked.as_slice(), expected_chars);

        let sparkle_heart_bytes = vec![240_u8, 159, 146, 150];
        let cstring = CString::new(sparkle_heart_bytes).unwrap();
        let cstr = &*cstring;
        let ascii_err = cstr.into_ascii_string().unwrap_err();
        assert_eq!(ascii_err.into_source(), &*cstring);
    }

    #[test]
    #[cfg(feature = "std")]
    fn fmt_ascii_string() {
        let s = "abc".to_string().into_ascii_string().unwrap();
        assert_eq!(format!("{}", s), "abc".to_string());
        assert_eq!(format!("{:?}", s), "\"abc\"".to_string());
    }

    #[test]
    fn write_fmt() {
        use alloc::{fmt, str};

        let mut s0 = AsciiString::new();
        fmt::write(&mut s0, format_args!("Hello World")).unwrap();
        assert_eq!(s0, "Hello World");

        let mut s1 = AsciiString::new();
        fmt::write(&mut s1, format_args!("{}", 9)).unwrap();
        assert_eq!(s1, "9");

        let mut s2 = AsciiString::new();
        let sparkle_heart_bytes = [240, 159, 146, 150];
        let sparkle_heart = str::from_utf8(&sparkle_heart_bytes).unwrap();
        assert!(fmt::write(&mut s2, format_args!("{}", sparkle_heart)).is_err());
    }

    #[test]
    fn to_and_from_box() {
        let string = "abc".into_ascii_string().unwrap();
        let converted: Box<AsciiStr> = Box::from(string.clone());
        let converted: AsciiString = converted.into();
        assert_eq!(string, converted);
    }
}
