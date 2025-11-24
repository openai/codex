#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::fmt;
use core::ops::{Index, IndexMut};
use core::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};
use core::slice::{self, Iter, IterMut, SliceIndex};
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(feature = "std")]
use std::ffi::CStr;

use ascii_char::AsciiChar;
#[cfg(feature = "alloc")]
use ascii_string::AsciiString;

/// [`AsciiStr`] represents a byte or string slice that only contains ASCII characters.
///
/// It wraps an `[AsciiChar]` and implements many of `str`s methods and traits.
///
/// It can be created by a checked conversion from a `str` or `[u8]`, or borrowed from an
/// `AsciiString`.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AsciiStr {
    slice: [AsciiChar],
}

impl AsciiStr {
    /// Converts `&self` to a `&str` slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: All variants of `AsciiChar` are valid bytes for a `str`.
        unsafe { &*(self as *const AsciiStr as *const str) }
    }

    /// Converts `&self` into a byte slice.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: All variants of `AsciiChar` are valid `u8`, given they're `repr(u8)`.
        unsafe { &*(self as *const AsciiStr as *const [u8]) }
    }

    /// Returns the entire string as slice of `AsciiChar`s.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[AsciiChar] {
        &self.slice
    }

    /// Returns the entire string as mutable slice of `AsciiChar`s.
    #[inline]
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [AsciiChar] {
        &mut self.slice
    }

    /// Returns a raw pointer to the `AsciiStr`'s buffer.
    ///
    /// The caller must ensure that the slice outlives the pointer this function returns, or else it
    /// will end up pointing to garbage. Modifying the `AsciiStr` may cause it's buffer to be
    /// reallocated, which would also make any pointers to it invalid.
    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *const AsciiChar {
        self.as_slice().as_ptr()
    }

    /// Returns an unsafe mutable pointer to the `AsciiStr`'s buffer.
    ///
    /// The caller must ensure that the slice outlives the pointer this function returns, or else it
    /// will end up pointing to garbage. Modifying the `AsciiStr` may cause it's buffer to be
    /// reallocated, which would also make any pointers to it invalid.
    #[inline]
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut AsciiChar {
        self.as_mut_slice().as_mut_ptr()
    }

    /// Copies the content of this `AsciiStr` into an owned `AsciiString`.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_ascii_string(&self) -> AsciiString {
        AsciiString::from(self.slice.to_vec())
    }

    /// Converts anything that can represent a byte slice into an `AsciiStr`.
    ///
    /// # Errors
    /// If `bytes` contains a non-ascii byte, `Err` will be returned
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let foo = AsciiStr::from_ascii(b"foo");
    /// let err = AsciiStr::from_ascii("Ŋ");
    /// assert_eq!(foo.unwrap().as_str(), "foo");
    /// assert_eq!(err.unwrap_err().valid_up_to(), 0);
    /// ```
    #[inline]
    pub fn from_ascii<B>(bytes: &B) -> Result<&AsciiStr, AsAsciiStrError>
    where
        B: AsRef<[u8]> + ?Sized,
    {
        bytes.as_ref().as_ascii_str()
    }

    /// Converts anything that can be represented as a byte slice to an `AsciiStr` without checking
    /// for non-ASCII characters..
    ///
    /// # Safety
    /// If any of the bytes in `bytes` do not represent valid ascii characters, calling
    /// this function is undefined behavior.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let foo = unsafe { AsciiStr::from_ascii_unchecked(&b"foo"[..]) };
    /// assert_eq!(foo.as_str(), "foo");
    /// ```
    #[inline]
    #[must_use]
    pub unsafe fn from_ascii_unchecked(bytes: &[u8]) -> &AsciiStr {
        // SAFETY: Caller guarantees all bytes in `bytes` are valid
        //         ascii characters.
        unsafe { bytes.as_ascii_str_unchecked() }
    }

    /// Returns the number of characters / bytes in this ASCII sequence.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let s = AsciiStr::from_ascii("foo").unwrap();
    /// assert_eq!(s.len(), 3);
    /// ```
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.slice.len()
    }

    /// Returns true if the ASCII slice contains zero bytes.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let mut empty = AsciiStr::from_ascii("").unwrap();
    /// let mut full = AsciiStr::from_ascii("foo").unwrap();
    /// assert!(empty.is_empty());
    /// assert!(!full.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the characters of the `AsciiStr`.
    #[inline]
    #[must_use]
    pub fn chars(&self) -> Chars {
        Chars(self.slice.iter())
    }

    /// Returns an iterator over the characters of the `AsciiStr` which allows you to modify the
    /// value of each `AsciiChar`.
    #[inline]
    #[must_use]
    pub fn chars_mut(&mut self) -> CharsMut {
        CharsMut(self.slice.iter_mut())
    }

    /// Returns an iterator over parts of the `AsciiStr` separated by a character.
    ///
    /// # Examples
    /// ```
    /// # use ascii::{AsciiStr, AsciiChar};
    /// let words = AsciiStr::from_ascii("apple banana lemon").unwrap()
    ///     .split(AsciiChar::Space)
    ///     .map(|a| a.as_str())
    ///     .collect::<Vec<_>>();
    /// assert_eq!(words, ["apple", "banana", "lemon"]);
    /// ```
    #[must_use]
    pub fn split(&self, on: AsciiChar) -> impl DoubleEndedIterator<Item = &AsciiStr> {
        Split {
            on,
            ended: false,
            chars: self.chars(),
        }
    }

    /// Returns an iterator over the lines of the `AsciiStr`, which are themselves `AsciiStr`s.
    ///
    /// Lines are ended with either `LineFeed` (`\n`), or `CarriageReturn` then `LineFeed` (`\r\n`).
    ///
    /// The final line ending is optional.
    #[inline]
    #[must_use]
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &AsciiStr> {
        Lines { string: self }
    }

    /// Returns an ASCII string slice with leading and trailing whitespace removed.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let example = AsciiStr::from_ascii("  \twhite \tspace  \t").unwrap();
    /// assert_eq!("white \tspace", example.trim());
    /// ```
    #[must_use]
    pub fn trim(&self) -> &Self {
        self.trim_start().trim_end()
    }

    /// Returns an ASCII string slice with leading whitespace removed.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let example = AsciiStr::from_ascii("  \twhite \tspace  \t").unwrap();
    /// assert_eq!("white \tspace  \t", example.trim_start());
    /// ```
    #[must_use]
    pub fn trim_start(&self) -> &Self {
        let whitespace_len = self
            .chars()
            .position(|ch| !ch.is_whitespace())
            .unwrap_or_else(|| self.len());

        // SAFETY: `whitespace_len` is `0..=len`, which is at most `len`, which is a valid empty slice.
        unsafe { self.as_slice().get_unchecked(whitespace_len..).into() }
    }

    /// Returns an ASCII string slice with trailing whitespace removed.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiStr;
    /// let example = AsciiStr::from_ascii("  \twhite \tspace  \t").unwrap();
    /// assert_eq!("  \twhite \tspace", example.trim_end());
    /// ```
    #[must_use]
    pub fn trim_end(&self) -> &Self {
        // Number of whitespace characters counting from the end
        let whitespace_len = self
            .chars()
            .rev()
            .position(|ch| !ch.is_whitespace())
            .unwrap_or_else(|| self.len());

        // SAFETY: `whitespace_len` is `0..=len`, which is at most `len`, which is a valid empty slice, and at least `0`, which is the whole slice.
        unsafe {
            self.as_slice()
                .get_unchecked(..self.len() - whitespace_len)
                .into()
        }
    }

    /// Compares two strings case-insensitively.
    #[must_use]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .chars()
                .zip(other.chars())
                .all(|(ch, other_ch)| ch.eq_ignore_ascii_case(&other_ch))
    }

    /// Replaces lowercase letters with their uppercase equivalent.
    pub fn make_ascii_uppercase(&mut self) {
        for ch in self.chars_mut() {
            *ch = ch.to_ascii_uppercase();
        }
    }

    /// Replaces uppercase letters with their lowercase equivalent.
    pub fn make_ascii_lowercase(&mut self) {
        for ch in self.chars_mut() {
            *ch = ch.to_ascii_lowercase();
        }
    }

    /// Returns a copy of this string where letters 'a' to 'z' are mapped to 'A' to 'Z'.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_ascii_uppercase(&self) -> AsciiString {
        let mut ascii_string = self.to_ascii_string();
        ascii_string.make_ascii_uppercase();
        ascii_string
    }

    /// Returns a copy of this string where letters 'A' to 'Z' are mapped to 'a' to 'z'.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_ascii_lowercase(&self) -> AsciiString {
        let mut ascii_string = self.to_ascii_string();
        ascii_string.make_ascii_lowercase();
        ascii_string
    }

    /// Returns the first character if the string is not empty.
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<AsciiChar> {
        self.slice.first().copied()
    }

    /// Returns the last character if the string is not empty.
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<AsciiChar> {
        self.slice.last().copied()
    }

    /// Converts a [`Box<AsciiStr>`] into a [`AsciiString`] without copying or allocating.
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn into_ascii_string(self: Box<Self>) -> AsciiString {
        let slice = Box::<[AsciiChar]>::from(self);
        AsciiString::from(slice.into_vec())
    }
}

macro_rules! impl_partial_eq {
    ($wider: ty) => {
        impl PartialEq<$wider> for AsciiStr {
            #[inline]
            fn eq(&self, other: &$wider) -> bool {
                <AsciiStr as AsRef<$wider>>::as_ref(self) == other
            }
        }
        impl PartialEq<AsciiStr> for $wider {
            #[inline]
            fn eq(&self, other: &AsciiStr) -> bool {
                self == <AsciiStr as AsRef<$wider>>::as_ref(other)
            }
        }
    };
}

impl_partial_eq! {str}
impl_partial_eq! {[u8]}
impl_partial_eq! {[AsciiChar]}

#[cfg(feature = "alloc")]
impl ToOwned for AsciiStr {
    type Owned = AsciiString;

    #[inline]
    fn to_owned(&self) -> AsciiString {
        self.to_ascii_string()
    }
}

impl AsRef<[u8]> for AsciiStr {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl AsRef<str> for AsciiStr {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<[AsciiChar]> for AsciiStr {
    #[inline]
    fn as_ref(&self) -> &[AsciiChar] {
        &self.slice
    }
}
impl AsMut<[AsciiChar]> for AsciiStr {
    #[inline]
    fn as_mut(&mut self) -> &mut [AsciiChar] {
        &mut self.slice
    }
}

impl Default for &'static AsciiStr {
    #[inline]
    fn default() -> &'static AsciiStr {
        From::from(&[] as &[AsciiChar])
    }
}
impl<'a> From<&'a [AsciiChar]> for &'a AsciiStr {
    #[inline]
    fn from(slice: &[AsciiChar]) -> &AsciiStr {
        let ptr = slice as *const [AsciiChar] as *const AsciiStr;
        unsafe { &*ptr }
    }
}
impl<'a> From<&'a mut [AsciiChar]> for &'a mut AsciiStr {
    #[inline]
    fn from(slice: &mut [AsciiChar]) -> &mut AsciiStr {
        let ptr = slice as *mut [AsciiChar] as *mut AsciiStr;
        unsafe { &mut *ptr }
    }
}
#[cfg(feature = "alloc")]
impl From<Box<[AsciiChar]>> for Box<AsciiStr> {
    #[inline]
    fn from(owned: Box<[AsciiChar]>) -> Box<AsciiStr> {
        let ptr = Box::into_raw(owned) as *mut AsciiStr;
        unsafe { Box::from_raw(ptr) }
    }
}

impl AsRef<AsciiStr> for AsciiStr {
    #[inline]
    fn as_ref(&self) -> &AsciiStr {
        self
    }
}
impl AsMut<AsciiStr> for AsciiStr {
    #[inline]
    fn as_mut(&mut self) -> &mut AsciiStr {
        self
    }
}
impl AsRef<AsciiStr> for [AsciiChar] {
    #[inline]
    fn as_ref(&self) -> &AsciiStr {
        self.into()
    }
}
impl AsMut<AsciiStr> for [AsciiChar] {
    #[inline]
    fn as_mut(&mut self) -> &mut AsciiStr {
        self.into()
    }
}

impl<'a> From<&'a AsciiStr> for &'a [AsciiChar] {
    #[inline]
    fn from(astr: &AsciiStr) -> &[AsciiChar] {
        &astr.slice
    }
}
impl<'a> From<&'a mut AsciiStr> for &'a mut [AsciiChar] {
    #[inline]
    fn from(astr: &mut AsciiStr) -> &mut [AsciiChar] {
        &mut astr.slice
    }
}
impl<'a> From<&'a AsciiStr> for &'a [u8] {
    #[inline]
    fn from(astr: &AsciiStr) -> &[u8] {
        astr.as_bytes()
    }
}
impl<'a> From<&'a AsciiStr> for &'a str {
    #[inline]
    fn from(astr: &AsciiStr) -> &str {
        astr.as_str()
    }
}
macro_rules! widen_box {
    ($wider: ty) => {
        #[cfg(feature = "alloc")]
        impl From<Box<AsciiStr>> for Box<$wider> {
            #[inline]
            fn from(owned: Box<AsciiStr>) -> Box<$wider> {
                let ptr = Box::into_raw(owned) as *mut $wider;
                unsafe { Box::from_raw(ptr) }
            }
        }
    };
}
widen_box! {[AsciiChar]}
widen_box! {[u8]}
widen_box! {str}

// allows &AsciiChar to be used by generic AsciiString Extend and FromIterator
impl AsRef<AsciiStr> for AsciiChar {
    fn as_ref(&self) -> &AsciiStr {
        slice::from_ref(self).into()
    }
}

impl fmt::Display for AsciiStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for AsciiStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

macro_rules! impl_index {
    ($idx:ty) => {
        #[allow(clippy::indexing_slicing)] // In `Index`, if it's out of bounds, panic is the default
        impl Index<$idx> for AsciiStr {
            type Output = AsciiStr;

            #[inline]
            fn index(&self, index: $idx) -> &AsciiStr {
                self.slice[index].as_ref()
            }
        }

        #[allow(clippy::indexing_slicing)] // In `IndexMut`, if it's out of bounds, panic is the default
        impl IndexMut<$idx> for AsciiStr {
            #[inline]
            fn index_mut(&mut self, index: $idx) -> &mut AsciiStr {
                self.slice[index].as_mut()
            }
        }
    };
}

impl_index! { Range<usize> }
impl_index! { RangeTo<usize> }
impl_index! { RangeFrom<usize> }
impl_index! { RangeFull }
impl_index! { RangeInclusive<usize> }
impl_index! { RangeToInclusive<usize> }

#[allow(clippy::indexing_slicing)] // In `Index`, if it's out of bounds, panic is the default
impl Index<usize> for AsciiStr {
    type Output = AsciiChar;

    #[inline]
    fn index(&self, index: usize) -> &AsciiChar {
        &self.slice[index]
    }
}

#[allow(clippy::indexing_slicing)] // In `IndexMut`, if it's out of bounds, panic is the default
impl IndexMut<usize> for AsciiStr {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut AsciiChar {
        &mut self.slice[index]
    }
}

/// Produces references for compatibility with `[u8]`.
///
/// (`str` doesn't implement `IntoIterator` for its references,
///  so there is no compatibility to lose.)
impl<'a> IntoIterator for &'a AsciiStr {
    type Item = &'a AsciiChar;
    type IntoIter = CharsRef<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        CharsRef(self.as_slice().iter())
    }
}

impl<'a> IntoIterator for &'a mut AsciiStr {
    type Item = &'a mut AsciiChar;
    type IntoIter = CharsMut<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.chars_mut()
    }
}

/// A copying iterator over the characters of an `AsciiStr`.
#[derive(Clone, Debug)]
pub struct Chars<'a>(Iter<'a, AsciiChar>);
impl<'a> Chars<'a> {
    /// Returns the ascii string slice with the remaining characters.
    #[must_use]
    pub fn as_str(&self) -> &'a AsciiStr {
        self.0.as_slice().into()
    }
}
impl<'a> Iterator for Chars<'a> {
    type Item = AsciiChar;
    #[inline]
    fn next(&mut self) -> Option<AsciiChar> {
        self.0.next().copied()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<'a> DoubleEndedIterator for Chars<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<AsciiChar> {
        self.0.next_back().copied()
    }
}
impl<'a> ExactSizeIterator for Chars<'a> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// A mutable iterator over the characters of an `AsciiStr`.
#[derive(Debug)]
pub struct CharsMut<'a>(IterMut<'a, AsciiChar>);
impl<'a> CharsMut<'a> {
    /// Returns the ascii string slice with the remaining characters.
    #[must_use]
    pub fn into_str(self) -> &'a mut AsciiStr {
        self.0.into_slice().into()
    }
}
impl<'a> Iterator for CharsMut<'a> {
    type Item = &'a mut AsciiChar;
    #[inline]
    fn next(&mut self) -> Option<&'a mut AsciiChar> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<'a> DoubleEndedIterator for CharsMut<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut AsciiChar> {
        self.0.next_back()
    }
}
impl<'a> ExactSizeIterator for CharsMut<'a> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// An immutable iterator over the characters of an `AsciiStr`.
#[derive(Clone, Debug)]
pub struct CharsRef<'a>(Iter<'a, AsciiChar>);
impl<'a> CharsRef<'a> {
    /// Returns the ascii string slice with the remaining characters.
    #[must_use]
    pub fn as_str(&self) -> &'a AsciiStr {
        self.0.as_slice().into()
    }
}
impl<'a> Iterator for CharsRef<'a> {
    type Item = &'a AsciiChar;
    #[inline]
    fn next(&mut self) -> Option<&'a AsciiChar> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<'a> DoubleEndedIterator for CharsRef<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a AsciiChar> {
        self.0.next_back()
    }
}

/// An iterator over parts of an `AsciiStr` separated by an `AsciiChar`.
///
/// This type is created by [`AsciiChar::split()`](struct.AsciiChar.html#method.split).
#[derive(Clone, Debug)]
struct Split<'a> {
    on: AsciiChar,
    ended: bool,
    chars: Chars<'a>,
}
impl<'a> Iterator for Split<'a> {
    type Item = &'a AsciiStr;

    fn next(&mut self) -> Option<&'a AsciiStr> {
        if !self.ended {
            let start: &AsciiStr = self.chars.as_str();
            let split_on = self.on;

            if let Some(at) = self.chars.position(|ch| ch == split_on) {
                // SAFETY: `at` is guaranteed to be in bounds, as `position` returns `Ok(0..len)`.
                Some(unsafe { start.as_slice().get_unchecked(..at).into() })
            } else {
                self.ended = true;
                Some(start)
            }
        } else {
            None
        }
    }
}
impl<'a> DoubleEndedIterator for Split<'a> {
    fn next_back(&mut self) -> Option<&'a AsciiStr> {
        if !self.ended {
            let start: &AsciiStr = self.chars.as_str();
            let split_on = self.on;

            if let Some(at) = self.chars.rposition(|ch| ch == split_on) {
                // SAFETY: `at` is guaranteed to be in bounds, as `rposition` returns `Ok(0..len)`, and slices `1..`, `2..`, etc... until `len..` inclusive, are valid.
                Some(unsafe { start.as_slice().get_unchecked(at + 1..).into() })
            } else {
                self.ended = true;
                Some(start)
            }
        } else {
            None
        }
    }
}

/// An iterator over the lines of the internal character array.
#[derive(Clone, Debug)]
struct Lines<'a> {
    string: &'a AsciiStr,
}
impl<'a> Iterator for Lines<'a> {
    type Item = &'a AsciiStr;

    fn next(&mut self) -> Option<&'a AsciiStr> {
        if let Some(idx) = self
            .string
            .chars()
            .position(|chr| chr == AsciiChar::LineFeed)
        {
            // SAFETY: `idx` is guaranteed to be `1..len`, as we get it from `position` as `0..len` and make sure it's not `0`.
            let line = if idx > 0
                && *unsafe { self.string.as_slice().get_unchecked(idx - 1) }
                    == AsciiChar::CarriageReturn
            {
                // SAFETY: As per above, `idx` is guaranteed to be `1..len`
                unsafe { self.string.as_slice().get_unchecked(..idx - 1).into() }
            } else {
                // SAFETY: As per above, `idx` is guaranteed to be `0..len`
                unsafe { self.string.as_slice().get_unchecked(..idx).into() }
            };
            // SAFETY: As per above, `idx` is guaranteed to be `0..len`, so at the extreme, slicing `len..` is a valid empty slice.
            self.string = unsafe { self.string.as_slice().get_unchecked(idx + 1..).into() };
            Some(line)
        } else if self.string.is_empty() {
            None
        } else {
            let line = self.string;
            // SAFETY: An empty string is a valid string.
            self.string = unsafe { AsciiStr::from_ascii_unchecked(b"") };
            Some(line)
        }
    }
}

impl<'a> DoubleEndedIterator for Lines<'a> {
    fn next_back(&mut self) -> Option<&'a AsciiStr> {
        if self.string.is_empty() {
            return None;
        }

        // If we end with `LF` / `CR/LF`, remove them
        if let Some(AsciiChar::LineFeed) = self.string.last() {
            // SAFETY: `last()` returned `Some`, so our len is at least 1.
            self.string = unsafe {
                self.string
                    .as_slice()
                    .get_unchecked(..self.string.len() - 1)
                    .into()
            };

            if let Some(AsciiChar::CarriageReturn) = self.string.last() {
                // SAFETY: `last()` returned `Some`, so our len is at least 1.
                self.string = unsafe {
                    self.string
                        .as_slice()
                        .get_unchecked(..self.string.len() - 1)
                        .into()
                };
            }
        }

        // Get the position of the first `LF` from the end.
        let lf_rev_pos = self
            .string
            .chars()
            .rev()
            .position(|ch| ch == AsciiChar::LineFeed)
            .unwrap_or_else(|| self.string.len());

        // SAFETY: `lf_rev_pos` will be in range `0..=len`, so `len - lf_rev_pos`
        //         will be within `0..=len`, making it correct as a start and end
        //         point for the strings.
        let line = unsafe {
            self.string
                .as_slice()
                .get_unchecked(self.string.len() - lf_rev_pos..)
                .into()
        };
        self.string = unsafe {
            self.string
                .as_slice()
                .get_unchecked(..self.string.len() - lf_rev_pos)
                .into()
        };
        Some(line)
    }
}

/// Error that is returned when a sequence of `u8` are not all ASCII.
///
/// Is used by `As[Mut]AsciiStr` and the `from_ascii` method on `AsciiStr` and `AsciiString`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AsAsciiStrError(usize);

const ERRORMSG_STR: &str = "one or more bytes are not ASCII";

impl AsAsciiStrError {
    /// Returns the index of the first non-ASCII byte.
    ///
    /// It is the maximum index such that `from_ascii(input[..index])` would return `Ok(_)`.
    #[inline]
    #[must_use]
    pub const fn valid_up_to(self) -> usize {
        self.0
    }
    #[cfg(not(feature = "std"))]
    /// Returns a description for this error, like `std::error::Error::description`.
    #[inline]
    #[must_use]
    #[allow(clippy::unused_self)]
    pub const fn description(&self) -> &'static str {
        ERRORMSG_STR
    }
}
impl fmt::Display for AsAsciiStrError {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        write!(fmtr, "the byte at index {} is not ASCII", self.0)
    }
}
#[cfg(feature = "std")]
impl Error for AsAsciiStrError {
    #[inline]
    fn description(&self) -> &'static str {
        ERRORMSG_STR
    }
}

/// Convert slices of bytes or [`AsciiChar`] to [`AsciiStr`].
// Could nearly replace this trait with SliceIndex, but its methods isn't even
// on a path for stabilization.
pub trait AsAsciiStr {
    /// Used to constrain `SliceIndex`
    #[doc(hidden)]
    type Inner;
    /// Convert a subslice to an ASCII slice.
    ///
    /// # Errors
    /// Returns `Err` if the range is out of bounds or if not all bytes in the
    /// slice are ASCII. The value in the error will be the index of the first
    /// non-ASCII byte or the end of the slice.
    ///
    /// # Examples
    /// ```
    /// use ascii::AsAsciiStr;
    /// assert!("'zoä'".slice_ascii(..3).is_ok());
    /// assert!("'zoä'".slice_ascii(0..4).is_err());
    /// assert!("'zoä'".slice_ascii(5..=5).is_ok());
    /// assert!("'zoä'".slice_ascii(4..).is_err());
    /// assert!(b"\r\n".slice_ascii(..).is_ok());
    /// ```
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[Self::Inner], Output = [Self::Inner]>;
    /// Convert to an ASCII slice.
    ///
    /// # Errors
    /// Returns `Err` if not all bytes are valid ascii values.
    ///
    /// # Example
    /// ```
    /// use ascii::{AsAsciiStr, AsciiChar};
    /// assert!("ASCII".as_ascii_str().is_ok());
    /// assert!(b"\r\n".as_ascii_str().is_ok());
    /// assert!("'zoä'".as_ascii_str().is_err());
    /// assert!(b"\xff".as_ascii_str().is_err());
    /// assert!([AsciiChar::C][..].as_ascii_str().is_ok()); // infallible
    /// ```
    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        self.slice_ascii(..)
    }
    /// Get a single ASCII character from the slice.
    ///
    /// Returns `None` if the index is out of bounds or the byte is not ASCII.
    ///
    /// # Examples
    /// ```
    /// use ascii::{AsAsciiStr, AsciiChar};
    /// assert_eq!("'zoä'".get_ascii(4), None);
    /// assert_eq!("'zoä'".get_ascii(5), Some(AsciiChar::Apostrophe));
    /// assert_eq!("'zoä'".get_ascii(6), None);
    /// ```
    fn get_ascii(&self, index: usize) -> Option<AsciiChar> {
        self.slice_ascii(index..=index)
            .ok()
            .and_then(AsciiStr::first)
    }
    /// Convert to an ASCII slice without checking for non-ASCII characters.
    ///
    /// # Safety
    /// Calling this function when `self` contains non-ascii characters is
    /// undefined behavior.
    ///
    /// # Examples
    ///
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr;
}

/// Convert mutable slices of bytes or [`AsciiChar`] to [`AsciiStr`].
pub trait AsMutAsciiStr: AsAsciiStr {
    /// Convert a subslice to an ASCII slice.
    ///
    /// # Errors
    /// This function returns `Err` if range is out of bounds, or if
    /// `self` contains non-ascii values
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[Self::Inner], Output = [Self::Inner]>;

    /// Convert to a mutable ASCII slice.
    ///
    /// # Errors
    /// This function returns `Err` if `self` contains non-ascii values
    fn as_mut_ascii_str(&mut self) -> Result<&mut AsciiStr, AsAsciiStrError> {
        self.slice_ascii_mut(..)
    }

    /// Convert to a mutable ASCII slice without checking for non-ASCII characters.
    ///
    /// # Safety
    /// Calling this function when `self` contains non-ascii characters is
    /// undefined behavior.
    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr;
}

// These generic implementations mirror the generic implementations for AsRef<T> in core.
impl<'a, T> AsAsciiStr for &'a T
where
    T: AsAsciiStr + ?Sized,
{
    type Inner = <T as AsAsciiStr>::Inner;
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[Self::Inner], Output = [Self::Inner]>,
    {
        <T as AsAsciiStr>::slice_ascii(*self, range)
    }

    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { <T as AsAsciiStr>::as_ascii_str_unchecked(*self) }
    }
}

impl<'a, T> AsAsciiStr for &'a mut T
where
    T: AsAsciiStr + ?Sized,
{
    type Inner = <T as AsAsciiStr>::Inner;
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[Self::Inner], Output = [Self::Inner]>,
    {
        <T as AsAsciiStr>::slice_ascii(*self, range)
    }

    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { <T as AsAsciiStr>::as_ascii_str_unchecked(*self) }
    }
}

impl<'a, T> AsMutAsciiStr for &'a mut T
where
    T: AsMutAsciiStr + ?Sized,
{
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[Self::Inner], Output = [Self::Inner]>,
    {
        <T as AsMutAsciiStr>::slice_ascii_mut(*self, range)
    }

    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { <T as AsMutAsciiStr>::as_mut_ascii_str_unchecked(*self) }
    }
}

impl AsAsciiStr for AsciiStr {
    type Inner = AsciiChar;

    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[AsciiChar], Output = [AsciiChar]>,
    {
        self.slice.slice_ascii(range)
    }

    #[inline]
    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        Ok(self)
    }

    #[inline]
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        self
    }

    #[inline]
    fn get_ascii(&self, index: usize) -> Option<AsciiChar> {
        self.slice.get_ascii(index)
    }
}
impl AsMutAsciiStr for AsciiStr {
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[AsciiChar], Output = [AsciiChar]>,
    {
        self.slice.slice_ascii_mut(range)
    }

    #[inline]
    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr {
        self
    }
}

impl AsAsciiStr for [AsciiChar] {
    type Inner = AsciiChar;
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[AsciiChar], Output = [AsciiChar]>,
    {
        match self.get(range) {
            Some(slice) => Ok(slice.into()),
            None => Err(AsAsciiStrError(self.len())),
        }
    }

    #[inline]
    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        Ok(self.into())
    }

    #[inline]
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        <&AsciiStr>::from(self)
    }

    #[inline]
    fn get_ascii(&self, index: usize) -> Option<AsciiChar> {
        self.get(index).copied()
    }
}
impl AsMutAsciiStr for [AsciiChar] {
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[AsciiChar], Output = [AsciiChar]>,
    {
        let len = self.len();
        match self.get_mut(range) {
            Some(slice) => Ok(slice.into()),
            None => Err(AsAsciiStrError(len)),
        }
    }
    #[inline]
    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr {
        <&mut AsciiStr>::from(self)
    }
}

impl AsAsciiStr for [u8] {
    type Inner = u8;

    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        if let Some(slice) = self.get(range) {
            slice.as_ascii_str().map_err(|AsAsciiStrError(not_ascii)| {
                let offset = slice.as_ptr() as usize - self.as_ptr() as usize;
                AsAsciiStrError(offset + not_ascii)
            })
        } else {
            Err(AsAsciiStrError(self.len()))
        }
    }

    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        // is_ascii is likely optimized
        if self.is_ascii() {
            // SAFETY: `is_ascii` guarantees all bytes are within ascii range.
            unsafe { Ok(self.as_ascii_str_unchecked()) }
        } else {
            Err(AsAsciiStrError(
                self.iter().take_while(|&b| b.is_ascii()).count(),
            ))
        }
    }

    #[inline]
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { &*(self as *const [u8] as *const AsciiStr) }
    }
}
impl AsMutAsciiStr for [u8] {
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        let (ptr, len) = (self.as_ptr(), self.len());
        if let Some(slice) = self.get_mut(range) {
            let slice_ptr = slice.as_ptr();
            slice
                .as_mut_ascii_str()
                .map_err(|AsAsciiStrError(not_ascii)| {
                    let offset = slice_ptr as usize - ptr as usize;
                    AsAsciiStrError(offset + not_ascii)
                })
        } else {
            Err(AsAsciiStrError(len))
        }
    }

    fn as_mut_ascii_str(&mut self) -> Result<&mut AsciiStr, AsAsciiStrError> {
        // is_ascii() is likely optimized
        if self.is_ascii() {
            // SAFETY: `is_ascii` guarantees all bytes are within ascii range.
            unsafe { Ok(self.as_mut_ascii_str_unchecked()) }
        } else {
            Err(AsAsciiStrError(
                self.iter().take_while(|&b| b.is_ascii()).count(),
            ))
        }
    }

    #[inline]
    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { &mut *(self as *mut [u8] as *mut AsciiStr) }
    }
}

impl AsAsciiStr for str {
    type Inner = u8;
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        self.as_bytes().slice_ascii(range)
    }
    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        self.as_bytes().as_ascii_str()
    }
    #[inline]
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { self.as_bytes().as_ascii_str_unchecked() }
    }
}
impl AsMutAsciiStr for str {
    fn slice_ascii_mut<R>(&mut self, range: R) -> Result<&mut AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        // SAFETY: We don't modify the reference in this function, and the caller may
        //         only modify it to include valid ascii characters.
        let bytes = unsafe { self.as_bytes_mut() };
        match bytes.get_mut(range) {
            // Valid ascii slice
            Some(slice) if slice.is_ascii() => {
                // SAFETY: All bytes are ascii, so this cast is valid
                let ptr = slice.as_mut_ptr().cast::<AsciiChar>();
                let len = slice.len();

                // SAFETY: The pointer is valid for `len` elements, as it came
                //         from a slice.
                unsafe {
                    let slice = core::slice::from_raw_parts_mut(ptr, len);
                    Ok(<&mut AsciiStr>::from(slice))
                }
            }
            Some(slice) => {
                let not_ascii_len = slice.iter().copied().take_while(u8::is_ascii).count();
                let offset = slice.as_ptr() as usize - self.as_ptr() as usize;

                Err(AsAsciiStrError(offset + not_ascii_len))
            }
            None => Err(AsAsciiStrError(self.len())),
        }
    }
    fn as_mut_ascii_str(&mut self) -> Result<&mut AsciiStr, AsAsciiStrError> {
        match self.bytes().position(|b| !b.is_ascii()) {
            Some(index) => Err(AsAsciiStrError(index)),
            // SAFETY: All bytes were iterated, and all were ascii
            None => unsafe { Ok(self.as_mut_ascii_str_unchecked()) },
        }
    }
    #[inline]
    unsafe fn as_mut_ascii_str_unchecked(&mut self) -> &mut AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        &mut *(self as *mut str as *mut AsciiStr)
    }
}

/// Note that the trailing null byte will be removed in the conversion.
#[cfg(feature = "std")]
impl AsAsciiStr for CStr {
    type Inner = u8;
    fn slice_ascii<R>(&self, range: R) -> Result<&AsciiStr, AsAsciiStrError>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        self.to_bytes().slice_ascii(range)
    }
    #[inline]
    fn as_ascii_str(&self) -> Result<&AsciiStr, AsAsciiStrError> {
        self.to_bytes().as_ascii_str()
    }
    #[inline]
    unsafe fn as_ascii_str_unchecked(&self) -> &AsciiStr {
        // SAFETY: Caller guarantees `self` does not contain non-ascii characters
        unsafe { self.to_bytes().as_ascii_str_unchecked() }
    }
}

#[cfg(test)]
mod tests {
    use super::{AsAsciiStr, AsAsciiStrError, AsMutAsciiStr, AsciiStr};
    #[cfg(feature = "alloc")]
    use alloc::string::{String, ToString};
    #[cfg(feature = "alloc")]
    use alloc::vec::Vec;
    use AsciiChar;

    /// Ensures that common types, `str`, `[u8]`, `AsciiStr` and their
    /// references, shared and mutable implement `AsAsciiStr`.
    #[test]
    fn generic_as_ascii_str() {
        // Generic function to ensure `C` implements `AsAsciiStr`
        fn generic<C: AsAsciiStr + ?Sized>(c: &C) -> Result<&AsciiStr, AsAsciiStrError> {
            c.as_ascii_str()
        }

        let arr = [AsciiChar::A];
        let ascii_str = arr.as_ref().into();
        let mut mut_arr = arr; // Note: We need a second copy to prevent overlapping mutable borrows.
        let mut_ascii_str = mut_arr.as_mut().into();
        let mut_arr_mut_ref: &mut [AsciiChar] = &mut [AsciiChar::A];
        let mut string_bytes = [b'A'];
        let string_mut = unsafe { core::str::from_utf8_unchecked_mut(&mut string_bytes) }; // SAFETY: 'A' is a valid string.
        let string_mut_bytes: &mut [u8] = &mut [b'A'];

        // Note: This is a trick because `rustfmt` doesn't support
        //       attributes on blocks yet.
        #[rustfmt::skip]
        let _ = [
            assert_eq!(generic::<str             >("A"              ), Ok(ascii_str)),
            assert_eq!(generic::<[u8]            >(&b"A"[..]        ), Ok(ascii_str)),
            assert_eq!(generic::<AsciiStr        >(ascii_str        ), Ok(ascii_str)),
            assert_eq!(generic::<[AsciiChar]     >(&arr             ), Ok(ascii_str)),
            assert_eq!(generic::<&str            >(&"A"             ), Ok(ascii_str)),
            assert_eq!(generic::<&[u8]           >(&&b"A"[..]       ), Ok(ascii_str)),
            assert_eq!(generic::<&AsciiStr       >(&ascii_str       ), Ok(ascii_str)),
            assert_eq!(generic::<&[AsciiChar]    >(&&arr[..]        ), Ok(ascii_str)),
            assert_eq!(generic::<&mut str        >(&string_mut      ), Ok(ascii_str)),
            assert_eq!(generic::<&mut [u8]       >(&string_mut_bytes), Ok(ascii_str)),
            assert_eq!(generic::<&mut AsciiStr   >(&mut_ascii_str   ), Ok(ascii_str)),
            assert_eq!(generic::<&mut [AsciiChar]>(&mut_arr_mut_ref ), Ok(ascii_str)),
        ];
    }

    #[cfg(feature = "std")]
    #[test]
    fn cstring_as_ascii_str() {
        use std::ffi::CString;
        fn generic<C: AsAsciiStr + ?Sized>(c: &C) -> Result<&AsciiStr, AsAsciiStrError> {
            c.as_ascii_str()
        }
        let arr = [AsciiChar::A];
        let ascii_str: &AsciiStr = arr.as_ref().into();
        let cstr = CString::new("A").unwrap();
        assert_eq!(generic(&*cstr), Ok(ascii_str));
    }

    #[test]
    fn generic_as_mut_ascii_str() {
        fn generic_mut<C: AsMutAsciiStr + ?Sized>(
            c: &mut C,
        ) -> Result<&mut AsciiStr, AsAsciiStrError> {
            c.as_mut_ascii_str()
        }

        let mut arr_mut = [AsciiChar::B];
        let mut ascii_str_mut: &mut AsciiStr = arr_mut.as_mut().into();
        // Need a second reference to prevent overlapping mutable borrows
        let mut arr_mut_2 = [AsciiChar::B];
        let ascii_str_mut_2: &mut AsciiStr = arr_mut_2.as_mut().into();
        assert_eq!(generic_mut(&mut ascii_str_mut), Ok(&mut *ascii_str_mut_2));
        assert_eq!(generic_mut(ascii_str_mut), Ok(&mut *ascii_str_mut_2));
    }

    #[test]
    fn as_ascii_str() {
        macro_rules! err {{$i:expr} => {Err(AsAsciiStrError($i))}}
        let s = "abčd";
        let b = s.as_bytes();
        assert_eq!(s.as_ascii_str(), err!(2));
        assert_eq!(b.as_ascii_str(), err!(2));
        let a: &AsciiStr = [AsciiChar::a, AsciiChar::b][..].as_ref();
        assert_eq!(s[..2].as_ascii_str(), Ok(a));
        assert_eq!(b[..2].as_ascii_str(), Ok(a));
        assert_eq!(s.slice_ascii(..2), Ok(a));
        assert_eq!(b.slice_ascii(..2), Ok(a));
        assert_eq!(s.slice_ascii(..=2), err!(2));
        assert_eq!(b.slice_ascii(..=2), err!(2));
        assert_eq!(s.get_ascii(4), Some(AsciiChar::d));
        assert_eq!(b.get_ascii(4), Some(AsciiChar::d));
        assert_eq!(s.get_ascii(3), None);
        assert_eq!(b.get_ascii(3), None);
        assert_eq!(s.get_ascii(b.len()), None);
        assert_eq!(b.get_ascii(b.len()), None);
        assert_eq!(a.get_ascii(0), Some(AsciiChar::a));
        assert_eq!(a.get_ascii(a.len()), None);
    }

    #[test]
    #[cfg(feature = "std")]
    fn cstr_as_ascii_str() {
        use std::ffi::CStr;
        macro_rules! err {{$i:expr} => {Err(AsAsciiStrError($i))}}
        let cstr = CStr::from_bytes_with_nul(b"a\xbbcde\xffg\0").unwrap();
        assert_eq!(cstr.as_ascii_str(), err!(1));
        assert_eq!(cstr.slice_ascii(2..), err!(5));
        assert_eq!(cstr.get_ascii(5), None);
        assert_eq!(cstr.get_ascii(6), Some(AsciiChar::g));
        assert_eq!(cstr.get_ascii(7), None);
        let ascii_slice = &[AsciiChar::X, AsciiChar::Y, AsciiChar::Z, AsciiChar::Null][..];
        let ascii_str: &AsciiStr = ascii_slice.as_ref();
        let cstr = CStr::from_bytes_with_nul(ascii_str.as_bytes()).unwrap();
        assert_eq!(cstr.slice_ascii(..2), Ok(&ascii_str[..2]));
        assert_eq!(cstr.as_ascii_str(), Ok(&ascii_str[..3]));
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn as_mut_ascii_str() {
        macro_rules! err {{$i:expr} => {Err(AsAsciiStrError($i))}}
        let mut s: String = "abčd".to_string();
        let mut b: Vec<u8> = s.clone().into();
        let mut first = [AsciiChar::a, AsciiChar::b];
        let mut second = [AsciiChar::d];
        assert_eq!(s.as_mut_ascii_str(), err!(2));
        assert_eq!(b.as_mut_ascii_str(), err!(2));
        assert_eq!(s.slice_ascii_mut(..), err!(2));
        assert_eq!(b.slice_ascii_mut(..), err!(2));
        assert_eq!(s[..2].as_mut_ascii_str(), Ok((&mut first[..]).into()));
        assert_eq!(b[..2].as_mut_ascii_str(), Ok((&mut first[..]).into()));
        assert_eq!(s.slice_ascii_mut(0..2), Ok((&mut first[..]).into()));
        assert_eq!(b.slice_ascii_mut(0..2), Ok((&mut first[..]).into()));
        assert_eq!(s.slice_ascii_mut(4..), Ok((&mut second[..]).into()));
        assert_eq!(b.slice_ascii_mut(4..), Ok((&mut second[..]).into()));
        assert_eq!(s.slice_ascii_mut(4..=10), err!(5));
        assert_eq!(b.slice_ascii_mut(4..=10), err!(5));
    }

    #[test]
    fn default() {
        let default: &'static AsciiStr = Default::default();
        assert!(default.is_empty());
    }

    #[test]
    #[allow(clippy::redundant_slicing)]
    fn index() {
        let mut arr = [AsciiChar::A, AsciiChar::B, AsciiChar::C, AsciiChar::D];
        {
            let a: &AsciiStr = arr[..].into();
            assert_eq!(a[..].as_slice(), &a.as_slice()[..]);
            assert_eq!(a[..4].as_slice(), &a.as_slice()[..4]);
            assert_eq!(a[4..].as_slice(), &a.as_slice()[4..]);
            assert_eq!(a[2..3].as_slice(), &a.as_slice()[2..3]);
            assert_eq!(a[..=3].as_slice(), &a.as_slice()[..=3]);
            assert_eq!(a[1..=1].as_slice(), &a.as_slice()[1..=1]);
        }
        let mut copy = arr;
        let a_mut: &mut AsciiStr = { &mut arr[..] }.into();
        assert_eq!(a_mut[..].as_mut_slice(), &mut copy[..]);
        assert_eq!(a_mut[..2].as_mut_slice(), &mut copy[..2]);
        assert_eq!(a_mut[3..].as_mut_slice(), &mut copy[3..]);
        assert_eq!(a_mut[4..4].as_mut_slice(), &mut copy[4..4]);
        assert_eq!(a_mut[..=0].as_mut_slice(), &mut copy[..=0]);
        assert_eq!(a_mut[0..=2].as_mut_slice(), &mut copy[0..=2]);
    }

    #[test]
    fn as_str() {
        let b = b"( ;";
        let v = AsciiStr::from_ascii(b).unwrap();
        assert_eq!(v.as_str(), "( ;");
        assert_eq!(AsRef::<str>::as_ref(v), "( ;");
    }

    #[test]
    fn as_bytes() {
        let b = b"( ;";
        let v = AsciiStr::from_ascii(b).unwrap();
        assert_eq!(v.as_bytes(), b"( ;");
        assert_eq!(AsRef::<[u8]>::as_ref(v), b"( ;");
    }

    #[test]
    fn make_ascii_case() {
        let mut bytes = ([b'a', b'@', b'A'], [b'A', b'@', b'a']);
        let a = bytes.0.as_mut_ascii_str().unwrap();
        let b = bytes.1.as_mut_ascii_str().unwrap();
        assert!(a.eq_ignore_ascii_case(b));
        assert!(b.eq_ignore_ascii_case(a));
        a.make_ascii_lowercase();
        b.make_ascii_uppercase();
        assert_eq!(a, "a@a");
        assert_eq!(b, "A@A");
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn to_ascii_case() {
        let bytes = ([b'a', b'@', b'A'], [b'A', b'@', b'a']);
        let a = bytes.0.as_ascii_str().unwrap();
        let b = bytes.1.as_ascii_str().unwrap();
        assert_eq!(a.to_ascii_lowercase().as_str(), "a@a");
        assert_eq!(a.to_ascii_uppercase().as_str(), "A@A");
        assert_eq!(b.to_ascii_lowercase().as_str(), "a@a");
        assert_eq!(b.to_ascii_uppercase().as_str(), "A@A");
    }

    #[test]
    fn chars_iter() {
        let chars = &[
            b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l', b'd', b'\0',
        ];
        let ascii = AsciiStr::from_ascii(chars).unwrap();
        for (achar, byte) in ascii.chars().zip(chars.iter().copied()) {
            assert_eq!(achar, byte);
        }
    }

    #[test]
    fn chars_iter_mut() {
        let chars = &mut [
            b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l', b'd', b'\0',
        ];
        let ascii = chars.as_mut_ascii_str().unwrap();
        *ascii.chars_mut().next().unwrap() = AsciiChar::H;
        assert_eq!(ascii[0], b'H');
    }

    #[test]
    fn lines_iter() {
        use core::iter::Iterator;

        let lines: [&str; 4] = ["foo", "bar", "", "baz"];
        let joined = "foo\r\nbar\n\nbaz\n";
        let ascii = AsciiStr::from_ascii(joined.as_bytes()).unwrap();
        for (asciiline, line) in ascii.lines().zip(&lines) {
            assert_eq!(asciiline, *line);
        }
        assert_eq!(ascii.lines().count(), lines.len());

        let lines: [&str; 4] = ["foo", "bar", "", "baz"];
        let joined = "foo\r\nbar\n\nbaz";
        let ascii = AsciiStr::from_ascii(joined.as_bytes()).unwrap();
        for (asciiline, line) in ascii.lines().zip(&lines) {
            assert_eq!(asciiline, *line);
        }
        assert_eq!(ascii.lines().count(), lines.len());

        let trailing_line_break = b"\n";
        let ascii = AsciiStr::from_ascii(&trailing_line_break).unwrap();
        let mut line_iter = ascii.lines();
        assert_eq!(line_iter.next(), Some(AsciiStr::from_ascii("").unwrap()));
        assert_eq!(line_iter.next(), None);

        let empty_lines = b"\n\r\n\n\r\n";
        let mut iter_count = 0;
        let ascii = AsciiStr::from_ascii(&empty_lines).unwrap();
        for line in ascii.lines() {
            iter_count += 1;
            assert!(line.is_empty());
        }
        assert_eq!(4, iter_count);
    }

    #[test]
    fn lines_iter_rev() {
        let joined = "foo\r\nbar\n\nbaz\n";
        let ascii = AsciiStr::from_ascii(joined.as_bytes()).unwrap();
        assert_eq!(ascii.lines().rev().count(), 4);
        assert_eq!(ascii.lines().rev().count(), joined.lines().rev().count());
        for (asciiline, line) in ascii.lines().rev().zip(joined.lines().rev()) {
            assert_eq!(asciiline, line);
        }
        let mut iter = ascii.lines();
        assert_eq!(iter.next(), Some("foo".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), Some("baz".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), Some("".as_ascii_str().unwrap()));
        assert_eq!(iter.next(), Some("bar".as_ascii_str().unwrap()));

        let empty_lines = b"\n\r\n\n\r\n";
        let mut iter_count = 0;
        let ascii = AsciiStr::from_ascii(&empty_lines).unwrap();
        for line in ascii.lines().rev() {
            iter_count += 1;
            assert!(line.is_empty());
        }
        assert_eq!(4, iter_count);
    }

    #[test]
    fn lines_iter_empty() {
        assert_eq!("".as_ascii_str().unwrap().lines().next(), None);
        assert_eq!("".as_ascii_str().unwrap().lines().next_back(), None);
        assert_eq!("".lines().next(), None);
    }

    #[test]
    fn split_str() {
        fn split_equals_str(haystack: &str, needle: char) {
            let mut strs = haystack.split(needle);
            let mut asciis = haystack
                .as_ascii_str()
                .unwrap()
                .split(AsciiChar::from_ascii(needle).unwrap())
                .map(AsciiStr::as_str);
            loop {
                assert_eq!(asciis.size_hint(), strs.size_hint());
                let (a, s) = (asciis.next(), strs.next());
                assert_eq!(a, s);
                if a == None {
                    break;
                }
            }
            // test fusedness if str's version is fused
            if strs.next() == None {
                assert_eq!(asciis.next(), None);
            }
        }
        split_equals_str("", '=');
        split_equals_str("1,2,3", ',');
        split_equals_str("foo;bar;baz;", ';');
        split_equals_str("|||", '|');
        split_equals_str(" a  b  c ", ' ');
    }

    #[test]
    fn split_str_rev() {
        let words = " foo  bar baz ";
        let ascii = words.as_ascii_str().unwrap();
        for (word, asciiword) in words
            .split(' ')
            .rev()
            .zip(ascii.split(AsciiChar::Space).rev())
        {
            assert_eq!(asciiword, word);
        }
        let mut iter = ascii.split(AsciiChar::Space);
        assert_eq!(iter.next(), Some("".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), Some("".as_ascii_str().unwrap()));
        assert_eq!(iter.next(), Some("foo".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), Some("baz".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), Some("bar".as_ascii_str().unwrap()));
        assert_eq!(iter.next(), Some("".as_ascii_str().unwrap()));
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn split_str_empty() {
        let empty = <&AsciiStr>::default();
        let mut iter = empty.split(AsciiChar::NAK);
        assert_eq!(iter.next(), Some(empty));
        assert_eq!(iter.next(), None);
        let mut iter = empty.split(AsciiChar::NAK);
        assert_eq!(iter.next_back(), Some(empty));
        assert_eq!(iter.next_back(), None);
        assert_eq!("".split('s').next(), Some("")); // str.split() also produces one element
    }

    #[test]
    #[cfg(feature = "std")]
    fn fmt_ascii_str() {
        let s = "abc".as_ascii_str().unwrap();
        assert_eq!(format!("{}", s), "abc".to_string());
        assert_eq!(format!("{:?}", s), "\"abc\"".to_string());
    }
}
