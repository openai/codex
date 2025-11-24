use core::{fmt, iter, ops, ptr};

use alloc::{borrow::Cow, string::String, vec, vec::Vec};

#[cfg(feature = "std")]
use std::{
    error,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::{
    ext_slice::ByteSlice,
    utf8::{self, Utf8Error},
};

/// Concatenate the elements given by the iterator together into a single
/// `Vec<u8>`.
///
/// The elements may be any type that can be cheaply converted into an `&[u8]`.
/// This includes, but is not limited to, `&str`, `&BStr` and `&[u8]` itself.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bstr;
///
/// let s = bstr::concat(&["foo", "bar", "baz"]);
/// assert_eq!(s, "foobarbaz".as_bytes());
/// ```
#[inline]
pub fn concat<T, I>(elements: I) -> Vec<u8>
where
    T: AsRef<[u8]>,
    I: IntoIterator<Item = T>,
{
    let mut dest = vec![];
    for element in elements {
        dest.push_str(element);
    }
    dest
}

/// Join the elements given by the iterator with the given separator into a
/// single `Vec<u8>`.
///
/// Both the separator and the elements may be any type that can be cheaply
/// converted into an `&[u8]`. This includes, but is not limited to,
/// `&str`, `&BStr` and `&[u8]` itself.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bstr;
///
/// let s = bstr::join(",", &["foo", "bar", "baz"]);
/// assert_eq!(s, "foo,bar,baz".as_bytes());
/// ```
#[inline]
pub fn join<B, T, I>(separator: B, elements: I) -> Vec<u8>
where
    B: AsRef<[u8]>,
    T: AsRef<[u8]>,
    I: IntoIterator<Item = T>,
{
    let mut it = elements.into_iter();
    let mut dest = vec![];
    match it.next() {
        None => return dest,
        Some(first) => {
            dest.push_str(first);
        }
    }
    for element in it {
        dest.push_str(&separator);
        dest.push_str(element);
    }
    dest
}

impl ByteVec for Vec<u8> {
    #[inline]
    fn as_vec(&self) -> &Vec<u8> {
        self
    }

    #[inline]
    fn as_vec_mut(&mut self) -> &mut Vec<u8> {
        self
    }

    #[inline]
    fn into_vec(self) -> Vec<u8> {
        self
    }
}

/// Ensure that callers cannot implement `ByteSlice` by making an
/// umplementable trait its super trait.
mod private {
    pub trait Sealed {}
}
impl private::Sealed for Vec<u8> {}

/// A trait that extends `Vec<u8>` with string oriented methods.
///
/// Note that when using the constructor methods, such as
/// `ByteVec::from_slice`, one should actually call them using the concrete
/// type. For example:
///
/// ```
/// use bstr::{B, ByteVec};
///
/// let s = Vec::from_slice(b"abc"); // NOT ByteVec::from_slice("...")
/// assert_eq!(s, B("abc"));
/// ```
///
/// This trait is sealed and cannot be implemented outside of `bstr`.
pub trait ByteVec: private::Sealed {
    /// A method for accessing the raw vector bytes of this type. This is
    /// always a no-op and callers shouldn't care about it. This only exists
    /// for making the extension trait work.
    #[doc(hidden)]
    fn as_vec(&self) -> &Vec<u8>;

    /// A method for accessing the raw vector bytes of this type, mutably. This
    /// is always a no-op and callers shouldn't care about it. This only exists
    /// for making the extension trait work.
    #[doc(hidden)]
    fn as_vec_mut(&mut self) -> &mut Vec<u8>;

    /// A method for consuming ownership of this vector. This is always a no-op
    /// and callers shouldn't care about it. This only exists for making the
    /// extension trait work.
    #[doc(hidden)]
    fn into_vec(self) -> Vec<u8>
    where
        Self: Sized;

    /// Create a new owned byte string from the given byte slice.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::{B, ByteVec};
    ///
    /// let s = Vec::from_slice(b"abc");
    /// assert_eq!(s, B("abc"));
    /// ```
    #[inline]
    fn from_slice<B: AsRef<[u8]>>(bytes: B) -> Vec<u8> {
        bytes.as_ref().to_vec()
    }

    /// Create a new byte string from an owned OS string.
    ///
    /// When the underlying bytes of OS strings are accessible, then this
    /// always succeeds and is zero cost. Otherwise, this returns the given
    /// `OsString` if it is not valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// use bstr::{B, ByteVec};
    ///
    /// let os_str = OsString::from("foo");
    /// let bs = Vec::from_os_string(os_str).expect("valid UTF-8");
    /// assert_eq!(bs, B("foo"));
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn from_os_string(os_str: OsString) -> Result<Vec<u8>, OsString> {
        #[cfg(unix)]
        #[inline]
        fn imp(os_str: OsString) -> Result<Vec<u8>, OsString> {
            use std::os::unix::ffi::OsStringExt;

            Ok(os_str.into_vec())
        }

        #[cfg(not(unix))]
        #[inline]
        fn imp(os_str: OsString) -> Result<Vec<u8>, OsString> {
            os_str.into_string().map(Vec::from)
        }

        imp(os_str)
    }

    /// Lossily create a new byte string from an OS string slice.
    ///
    /// When the underlying bytes of OS strings are accessible, then this is
    /// zero cost and always returns a slice. Otherwise, a UTF-8 check is
    /// performed and if the given OS string is not valid UTF-8, then it is
    /// lossily decoded into valid UTF-8 (with invalid bytes replaced by the
    /// Unicode replacement codepoint).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// use bstr::{B, ByteVec};
    ///
    /// let os_str = OsStr::new("foo");
    /// let bs = Vec::from_os_str_lossy(os_str);
    /// assert_eq!(bs, B("foo"));
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn from_os_str_lossy(os_str: &OsStr) -> Cow<'_, [u8]> {
        #[cfg(unix)]
        #[inline]
        fn imp(os_str: &OsStr) -> Cow<'_, [u8]> {
            use std::os::unix::ffi::OsStrExt;

            Cow::Borrowed(os_str.as_bytes())
        }

        #[cfg(not(unix))]
        #[inline]
        fn imp(os_str: &OsStr) -> Cow<'_, [u8]> {
            match os_str.to_string_lossy() {
                Cow::Borrowed(x) => Cow::Borrowed(x.as_bytes()),
                Cow::Owned(x) => Cow::Owned(Vec::from(x)),
            }
        }

        imp(os_str)
    }

    /// Create a new byte string from an owned file path.
    ///
    /// When the underlying bytes of paths are accessible, then this always
    /// succeeds and is zero cost. Otherwise, this returns the given `PathBuf`
    /// if it is not valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// use bstr::{B, ByteVec};
    ///
    /// let path = PathBuf::from("foo");
    /// let bs = Vec::from_path_buf(path).expect("must be valid UTF-8");
    /// assert_eq!(bs, B("foo"));
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn from_path_buf(path: PathBuf) -> Result<Vec<u8>, PathBuf> {
        Vec::from_os_string(path.into_os_string()).map_err(PathBuf::from)
    }

    /// Lossily create a new byte string from a file path.
    ///
    /// When the underlying bytes of paths are accessible, then this is
    /// zero cost and always returns a slice. Otherwise, a UTF-8 check is
    /// performed and if the given path is not valid UTF-8, then it is lossily
    /// decoded into valid UTF-8 (with invalid bytes replaced by the Unicode
    /// replacement codepoint).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// use bstr::{B, ByteVec};
    ///
    /// let path = Path::new("foo");
    /// let bs = Vec::from_path_lossy(path);
    /// assert_eq!(bs, B("foo"));
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn from_path_lossy(path: &Path) -> Cow<'_, [u8]> {
        Vec::from_os_str_lossy(path.as_os_str())
    }

    /// Unescapes the given string into its raw bytes.
    ///
    /// This looks for the escape sequences `\xNN`, `\0`, `\r`, `\n`, `\t`
    /// and `\` and translates them into their corresponding unescaped form.
    ///
    /// Incomplete escape sequences or things that look like escape sequences
    /// but are not (for example, `\i` or `\xYZ`) are passed through literally.
    ///
    /// This is the dual of [`ByteSlice::escape_bytes`].
    ///
    /// Note that the zero or NUL byte may be represented as either `\0` or
    /// `\x00`. Both will be unescaped into the zero byte.
    ///
    /// # Examples
    ///
    /// This shows basic usage:
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use bstr::{B, BString, ByteVec};
    ///
    /// assert_eq!(
    ///     BString::from(b"foo\xFFbar"),
    ///     Vec::unescape_bytes(r"foo\xFFbar"),
    /// );
    /// assert_eq!(
    ///     BString::from(b"foo\nbar"),
    ///     Vec::unescape_bytes(r"foo\nbar"),
    /// );
    /// assert_eq!(
    ///     BString::from(b"foo\tbar"),
    ///     Vec::unescape_bytes(r"foo\tbar"),
    /// );
    /// assert_eq!(
    ///     BString::from(b"foo\\bar"),
    ///     Vec::unescape_bytes(r"foo\\bar"),
    /// );
    /// assert_eq!(
    ///     BString::from("foo☃bar"),
    ///     Vec::unescape_bytes(r"foo☃bar"),
    /// );
    ///
    /// # }
    /// ```
    ///
    /// This shows some examples of how incomplete or "incorrect" escape
    /// sequences get passed through literally.
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use bstr::{B, BString, ByteVec};
    ///
    /// // Show some incomplete escape sequences.
    /// assert_eq!(
    ///     BString::from(br"\"),
    ///     Vec::unescape_bytes(r"\"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\"),
    ///     Vec::unescape_bytes(r"\\"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\x"),
    ///     Vec::unescape_bytes(r"\x"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\xA"),
    ///     Vec::unescape_bytes(r"\xA"),
    /// );
    /// // And now some that kind of look like escape
    /// // sequences, but aren't.
    /// assert_eq!(
    ///     BString::from(br"\xZ"),
    ///     Vec::unescape_bytes(r"\xZ"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\xZZ"),
    ///     Vec::unescape_bytes(r"\xZZ"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\i"),
    ///     Vec::unescape_bytes(r"\i"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\u"),
    ///     Vec::unescape_bytes(r"\u"),
    /// );
    /// assert_eq!(
    ///     BString::from(br"\u{2603}"),
    ///     Vec::unescape_bytes(r"\u{2603}"),
    /// );
    ///
    /// # }
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    fn unescape_bytes<S: AsRef<str>>(escaped: S) -> Vec<u8> {
        let s = escaped.as_ref();
        crate::escape_bytes::UnescapeBytes::new(s.chars()).collect()
    }

    /// Appends the given byte to the end of this byte string.
    ///
    /// Note that this is equivalent to the generic `Vec::push` method. This
    /// method is provided to permit callers to explicitly differentiate
    /// between pushing bytes, codepoints and strings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = <Vec<u8>>::from("abc");
    /// s.push_byte(b'\xE2');
    /// s.push_byte(b'\x98');
    /// s.push_byte(b'\x83');
    /// assert_eq!(s, "abc☃".as_bytes());
    /// ```
    #[inline]
    fn push_byte(&mut self, byte: u8) {
        self.as_vec_mut().push(byte);
    }

    /// Appends the given `char` to the end of this byte string.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = <Vec<u8>>::from("abc");
    /// s.push_char('1');
    /// s.push_char('2');
    /// s.push_char('3');
    /// assert_eq!(s, "abc123".as_bytes());
    /// ```
    #[inline]
    fn push_char(&mut self, ch: char) {
        if ch.len_utf8() == 1 {
            self.push_byte(ch as u8);
            return;
        }
        self.as_vec_mut()
            .extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
    }

    /// Appends the given slice to the end of this byte string. This accepts
    /// any type that be converted to a `&[u8]`. This includes, but is not
    /// limited to, `&str`, `&BStr`, and of course, `&[u8]` itself.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = <Vec<u8>>::from("abc");
    /// s.push_str(b"123");
    /// assert_eq!(s, "abc123".as_bytes());
    /// ```
    #[inline]
    fn push_str<B: AsRef<[u8]>>(&mut self, bytes: B) {
        self.as_vec_mut().extend_from_slice(bytes.as_ref());
    }

    /// Converts a `Vec<u8>` into a `String` if and only if this byte string is
    /// valid UTF-8.
    ///
    /// If it is not valid UTF-8, then a
    /// [`FromUtf8Error`](struct.FromUtf8Error.html)
    /// is returned. (This error can be used to examine why UTF-8 validation
    /// failed, or to regain the original byte string.)
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let bytes = Vec::from("hello");
    /// let string = bytes.into_string().unwrap();
    ///
    /// assert_eq!("hello", string);
    /// ```
    ///
    /// If this byte string is not valid UTF-8, then an error will be returned.
    /// That error can then be used to inspect the location at which invalid
    /// UTF-8 was found, or to regain the original byte string:
    ///
    /// ```
    /// use bstr::{B, ByteVec};
    ///
    /// let bytes = Vec::from_slice(b"foo\xFFbar");
    /// let err = bytes.into_string().unwrap_err();
    ///
    /// assert_eq!(err.utf8_error().valid_up_to(), 3);
    /// assert_eq!(err.utf8_error().error_len(), Some(1));
    ///
    /// // At no point in this example is an allocation performed.
    /// let bytes = Vec::from(err.into_vec());
    /// assert_eq!(bytes, B(b"foo\xFFbar"));
    /// ```
    #[inline]
    fn into_string(self) -> Result<String, FromUtf8Error>
    where
        Self: Sized,
    {
        match utf8::validate(self.as_vec()) {
            Err(err) => Err(FromUtf8Error { original: self.into_vec(), err }),
            Ok(()) => {
                // SAFETY: This is safe because of the guarantees provided by
                // utf8::validate.
                unsafe { Ok(self.into_string_unchecked()) }
            }
        }
    }

    /// Lossily converts a `Vec<u8>` into a `String`. If this byte string
    /// contains invalid UTF-8, then the invalid bytes are replaced with the
    /// Unicode replacement codepoint.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let bytes = Vec::from_slice(b"foo\xFFbar");
    /// let string = bytes.into_string_lossy();
    /// assert_eq!(string, "foo\u{FFFD}bar");
    /// ```
    #[inline]
    fn into_string_lossy(self) -> String
    where
        Self: Sized,
    {
        match self.as_vec().to_str_lossy() {
            Cow::Borrowed(_) => {
                // SAFETY: to_str_lossy() returning a Cow::Borrowed guarantees
                // the entire string is valid utf8.
                unsafe { self.into_string_unchecked() }
            }
            Cow::Owned(s) => s,
        }
    }

    /// Unsafely convert this byte string into a `String`, without checking for
    /// valid UTF-8.
    ///
    /// # Safety
    ///
    /// Callers *must* ensure that this byte string is valid UTF-8 before
    /// calling this method. Converting a byte string into a `String` that is
    /// not valid UTF-8 is considered undefined behavior.
    ///
    /// This routine is useful in performance sensitive contexts where the
    /// UTF-8 validity of the byte string is already known and it is
    /// undesirable to pay the cost of an additional UTF-8 validation check
    /// that [`into_string`](#method.into_string) performs.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// // SAFETY: This is safe because string literals are guaranteed to be
    /// // valid UTF-8 by the Rust compiler.
    /// let s = unsafe { Vec::from("☃βツ").into_string_unchecked() };
    /// assert_eq!("☃βツ", s);
    /// ```
    #[inline]
    unsafe fn into_string_unchecked(self) -> String
    where
        Self: Sized,
    {
        String::from_utf8_unchecked(self.into_vec())
    }

    /// Converts this byte string into an OS string, in place.
    ///
    /// When OS strings can be constructed from arbitrary byte sequences, this
    /// always succeeds and is zero cost. Otherwise, if this byte string is not
    /// valid UTF-8, then an error (with the original byte string) is returned.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// use bstr::ByteVec;
    ///
    /// let bs = Vec::from("foo");
    /// let os_str = bs.into_os_string().expect("should be valid UTF-8");
    /// assert_eq!(os_str, OsStr::new("foo"));
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    fn into_os_string(self) -> Result<OsString, FromUtf8Error>
    where
        Self: Sized,
    {
        #[cfg(unix)]
        #[inline]
        fn imp(v: Vec<u8>) -> Result<OsString, FromUtf8Error> {
            use std::os::unix::ffi::OsStringExt;

            Ok(OsString::from_vec(v))
        }

        #[cfg(not(unix))]
        #[inline]
        fn imp(v: Vec<u8>) -> Result<OsString, FromUtf8Error> {
            v.into_string().map(OsString::from)
        }

        imp(self.into_vec())
    }

    /// Lossily converts this byte string into an OS string, in place.
    ///
    /// When OS strings can be constructed from arbitrary byte sequences, this
    /// is zero cost and always returns a slice. Otherwise, this will perform a
    /// UTF-8 check and lossily convert this byte string into valid UTF-8 using
    /// the Unicode replacement codepoint.
    ///
    /// Note that this can prevent the correct roundtripping of file paths when
    /// the representation of `OsString` is opaque.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let bs = Vec::from_slice(b"foo\xFFbar");
    /// let os_str = bs.into_os_string_lossy();
    /// assert_eq!(os_str.to_string_lossy(), "foo\u{FFFD}bar");
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn into_os_string_lossy(self) -> OsString
    where
        Self: Sized,
    {
        #[cfg(unix)]
        #[inline]
        fn imp(v: Vec<u8>) -> OsString {
            use std::os::unix::ffi::OsStringExt;

            OsString::from_vec(v)
        }

        #[cfg(not(unix))]
        #[inline]
        fn imp(v: Vec<u8>) -> OsString {
            OsString::from(v.into_string_lossy())
        }

        imp(self.into_vec())
    }

    /// Converts this byte string into an owned file path, in place.
    ///
    /// When paths can be constructed from arbitrary byte sequences, this
    /// always succeeds and is zero cost. Otherwise, if this byte string is not
    /// valid UTF-8, then an error (with the original byte string) is returned.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let bs = Vec::from("foo");
    /// let path = bs.into_path_buf().expect("should be valid UTF-8");
    /// assert_eq!(path.as_os_str(), "foo");
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    fn into_path_buf(self) -> Result<PathBuf, FromUtf8Error>
    where
        Self: Sized,
    {
        self.into_os_string().map(PathBuf::from)
    }

    /// Lossily converts this byte string into an owned file path, in place.
    ///
    /// When paths can be constructed from arbitrary byte sequences, this is
    /// zero cost and always returns a slice. Otherwise, this will perform a
    /// UTF-8 check and lossily convert this byte string into valid UTF-8 using
    /// the Unicode replacement codepoint.
    ///
    /// Note that this can prevent the correct roundtripping of file paths when
    /// the representation of `PathBuf` is opaque.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let bs = Vec::from_slice(b"foo\xFFbar");
    /// let path = bs.into_path_buf_lossy();
    /// assert_eq!(path.to_string_lossy(), "foo\u{FFFD}bar");
    /// ```
    #[inline]
    #[cfg(feature = "std")]
    fn into_path_buf_lossy(self) -> PathBuf
    where
        Self: Sized,
    {
        PathBuf::from(self.into_os_string_lossy())
    }

    /// Removes the last byte from this `Vec<u8>` and returns it.
    ///
    /// If this byte string is empty, then `None` is returned.
    ///
    /// If the last codepoint in this byte string is not ASCII, then removing
    /// the last byte could make this byte string contain invalid UTF-8.
    ///
    /// Note that this is equivalent to the generic `Vec::pop` method. This
    /// method is provided to permit callers to explicitly differentiate
    /// between popping bytes and codepoints.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foo");
    /// assert_eq!(s.pop_byte(), Some(b'o'));
    /// assert_eq!(s.pop_byte(), Some(b'o'));
    /// assert_eq!(s.pop_byte(), Some(b'f'));
    /// assert_eq!(s.pop_byte(), None);
    /// ```
    #[inline]
    fn pop_byte(&mut self) -> Option<u8> {
        self.as_vec_mut().pop()
    }

    /// Removes the last codepoint from this `Vec<u8>` and returns it.
    ///
    /// If this byte string is empty, then `None` is returned. If the last
    /// bytes of this byte string do not correspond to a valid UTF-8 code unit
    /// sequence, then the Unicode replacement codepoint is yielded instead in
    /// accordance with the
    /// [replacement codepoint substitution policy](index.html#handling-of-invalid-utf8-8).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foo");
    /// assert_eq!(s.pop_char(), Some('o'));
    /// assert_eq!(s.pop_char(), Some('o'));
    /// assert_eq!(s.pop_char(), Some('f'));
    /// assert_eq!(s.pop_char(), None);
    /// ```
    ///
    /// This shows the replacement codepoint substitution policy. Note that
    /// the first pop yields a replacement codepoint but actually removes two
    /// bytes. This is in contrast with subsequent pops when encountering
    /// `\xFF` since `\xFF` is never a valid prefix for any valid UTF-8
    /// code unit sequence.
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from_slice(b"f\xFF\xFF\xFFoo\xE2\x98");
    /// assert_eq!(s.pop_char(), Some('\u{FFFD}'));
    /// assert_eq!(s.pop_char(), Some('o'));
    /// assert_eq!(s.pop_char(), Some('o'));
    /// assert_eq!(s.pop_char(), Some('\u{FFFD}'));
    /// assert_eq!(s.pop_char(), Some('\u{FFFD}'));
    /// assert_eq!(s.pop_char(), Some('\u{FFFD}'));
    /// assert_eq!(s.pop_char(), Some('f'));
    /// assert_eq!(s.pop_char(), None);
    /// ```
    #[inline]
    fn pop_char(&mut self) -> Option<char> {
        let (ch, size) = utf8::decode_last_lossy(self.as_vec());
        if size == 0 {
            return None;
        }
        let new_len = self.as_vec().len() - size;
        self.as_vec_mut().truncate(new_len);
        Some(ch)
    }

    /// Removes a `char` from this `Vec<u8>` at the given byte position and
    /// returns it.
    ///
    /// If the bytes at the given position do not lead to a valid UTF-8 code
    /// unit sequence, then a
    /// [replacement codepoint is returned instead](index.html#handling-of-invalid-utf8-8).
    ///
    /// # Panics
    ///
    /// Panics if `at` is larger than or equal to this byte string's length.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foo☃bar");
    /// assert_eq!(s.remove_char(3), '☃');
    /// assert_eq!(s, b"foobar");
    /// ```
    ///
    /// This example shows how the Unicode replacement codepoint policy is
    /// used:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from_slice(b"foo\xFFbar");
    /// assert_eq!(s.remove_char(3), '\u{FFFD}');
    /// assert_eq!(s, b"foobar");
    /// ```
    #[inline]
    fn remove_char(&mut self, at: usize) -> char {
        let (ch, size) = utf8::decode_lossy(&self.as_vec()[at..]);
        assert!(
            size > 0,
            "expected {} to be less than {}",
            at,
            self.as_vec().len(),
        );
        self.as_vec_mut().drain(at..at + size);
        ch
    }

    /// Inserts the given codepoint into this `Vec<u8>` at a particular byte
    /// position.
    ///
    /// This is an `O(n)` operation as it may copy a number of elements in this
    /// byte string proportional to its length.
    ///
    /// # Panics
    ///
    /// Panics if `at` is larger than the byte string's length.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foobar");
    /// s.insert_char(3, '☃');
    /// assert_eq!(s, "foo☃bar".as_bytes());
    /// ```
    #[inline]
    fn insert_char(&mut self, at: usize, ch: char) {
        self.insert_str(at, ch.encode_utf8(&mut [0; 4]).as_bytes());
    }

    /// Inserts the given byte string into this byte string at a particular
    /// byte position.
    ///
    /// This is an `O(n)` operation as it may copy a number of elements in this
    /// byte string proportional to its length.
    ///
    /// The given byte string may be any type that can be cheaply converted
    /// into a `&[u8]`. This includes, but is not limited to, `&str` and
    /// `&[u8]`.
    ///
    /// # Panics
    ///
    /// Panics if `at` is larger than the byte string's length.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foobar");
    /// s.insert_str(3, "☃☃☃");
    /// assert_eq!(s, "foo☃☃☃bar".as_bytes());
    /// ```
    #[inline]
    fn insert_str<B: AsRef<[u8]>>(&mut self, at: usize, bytes: B) {
        let bytes = bytes.as_ref();
        let len = self.as_vec().len();
        assert!(at <= len, "expected {} to be <= {}", at, len);

        // SAFETY: We'd like to efficiently splice in the given bytes into
        // this byte string. Since we are only working with `u8` elements here,
        // we only need to consider whether our bounds are correct and whether
        // our byte string has enough space.
        self.as_vec_mut().reserve(bytes.len());
        unsafe {
            // Shift bytes after `at` over by the length of `bytes` to make
            // room for it. This requires referencing two regions of memory
            // that may overlap, so we use ptr::copy.
            ptr::copy(
                self.as_vec().as_ptr().add(at),
                self.as_vec_mut().as_mut_ptr().add(at + bytes.len()),
                len - at,
            );
            // Now copy the bytes given into the room we made above. In this
            // case, we know that the given bytes cannot possibly overlap
            // with this byte string since we have a mutable borrow of the
            // latter. Thus, we can use a nonoverlapping copy.
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.as_vec_mut().as_mut_ptr().add(at),
                bytes.len(),
            );
            self.as_vec_mut().set_len(len + bytes.len());
        }
    }

    /// Removes the specified range in this byte string and replaces it with
    /// the given bytes. The given bytes do not need to have the same length
    /// as the range provided.
    ///
    /// # Panics
    ///
    /// Panics if the given range is invalid.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foobar");
    /// s.replace_range(2..4, "xxxxx");
    /// assert_eq!(s, "foxxxxxar".as_bytes());
    /// ```
    #[inline]
    fn replace_range<R, B>(&mut self, range: R, replace_with: B)
    where
        R: ops::RangeBounds<usize>,
        B: AsRef<[u8]>,
    {
        self.as_vec_mut().splice(range, replace_with.as_ref().iter().copied());
    }

    /// Creates a draining iterator that removes the specified range in this
    /// `Vec<u8>` and yields each of the removed bytes.
    ///
    /// Note that the elements specified by the given range are removed
    /// regardless of whether the returned iterator is fully exhausted.
    ///
    /// Also note that is is unspecified how many bytes are removed from the
    /// `Vec<u8>` if the `DrainBytes` iterator is leaked.
    ///
    /// # Panics
    ///
    /// Panics if the given range is not valid.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::ByteVec;
    ///
    /// let mut s = Vec::from("foobar");
    /// {
    ///     let mut drainer = s.drain_bytes(2..4);
    ///     assert_eq!(drainer.next(), Some(b'o'));
    ///     assert_eq!(drainer.next(), Some(b'b'));
    ///     assert_eq!(drainer.next(), None);
    /// }
    /// assert_eq!(s, "foar".as_bytes());
    /// ```
    #[inline]
    fn drain_bytes<R>(&mut self, range: R) -> DrainBytes<'_>
    where
        R: ops::RangeBounds<usize>,
    {
        DrainBytes { it: self.as_vec_mut().drain(range) }
    }
}

/// A draining byte oriented iterator for `Vec<u8>`.
///
/// This iterator is created by
/// [`ByteVec::drain_bytes`](trait.ByteVec.html#method.drain_bytes).
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bstr::ByteVec;
///
/// let mut s = Vec::from("foobar");
/// {
///     let mut drainer = s.drain_bytes(2..4);
///     assert_eq!(drainer.next(), Some(b'o'));
///     assert_eq!(drainer.next(), Some(b'b'));
///     assert_eq!(drainer.next(), None);
/// }
/// assert_eq!(s, "foar".as_bytes());
/// ```
#[derive(Debug)]
pub struct DrainBytes<'a> {
    it: vec::Drain<'a, u8>,
}

impl<'a> iter::FusedIterator for DrainBytes<'a> {}

impl<'a> Iterator for DrainBytes<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        self.it.next()
    }
}

impl<'a> DoubleEndedIterator for DrainBytes<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<u8> {
        self.it.next_back()
    }
}

impl<'a> ExactSizeIterator for DrainBytes<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.it.len()
    }
}

/// An error that may occur when converting a `Vec<u8>` to a `String`.
///
/// This error includes the original `Vec<u8>` that failed to convert to a
/// `String`. This permits callers to recover the allocation used even if it
/// it not valid UTF-8.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bstr::{B, ByteVec};
///
/// let bytes = Vec::from_slice(b"foo\xFFbar");
/// let err = bytes.into_string().unwrap_err();
///
/// assert_eq!(err.utf8_error().valid_up_to(), 3);
/// assert_eq!(err.utf8_error().error_len(), Some(1));
///
/// // At no point in this example is an allocation performed.
/// let bytes = Vec::from(err.into_vec());
/// assert_eq!(bytes, B(b"foo\xFFbar"));
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct FromUtf8Error {
    original: Vec<u8>,
    err: Utf8Error,
}

impl FromUtf8Error {
    /// Return the original bytes as a slice that failed to convert to a
    /// `String`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::{B, ByteVec};
    ///
    /// let bytes = Vec::from_slice(b"foo\xFFbar");
    /// let err = bytes.into_string().unwrap_err();
    ///
    /// // At no point in this example is an allocation performed.
    /// assert_eq!(err.as_bytes(), B(b"foo\xFFbar"));
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.original
    }

    /// Consume this error and return the original byte string that failed to
    /// convert to a `String`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::{B, ByteVec};
    ///
    /// let bytes = Vec::from_slice(b"foo\xFFbar");
    /// let err = bytes.into_string().unwrap_err();
    /// let original = err.into_vec();
    ///
    /// // At no point in this example is an allocation performed.
    /// assert_eq!(original, B(b"foo\xFFbar"));
    /// ```
    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        self.original
    }

    /// Return the underlying UTF-8 error that occurred. This error provides
    /// information on the nature and location of the invalid UTF-8 detected.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bstr::{B, ByteVec};
    ///
    /// let bytes = Vec::from_slice(b"foo\xFFbar");
    /// let err = bytes.into_string().unwrap_err();
    ///
    /// assert_eq!(err.utf8_error().valid_up_to(), 3);
    /// assert_eq!(err.utf8_error().error_len(), Some(1));
    /// ```
    #[inline]
    pub fn utf8_error(&self) -> &Utf8Error {
        &self.err
    }
}

#[cfg(feature = "std")]
impl error::Error for FromUtf8Error {
    #[inline]
    fn description(&self) -> &str {
        "invalid UTF-8 vector"
    }
}

impl fmt::Display for FromUtf8Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.err)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::{vec, vec::Vec};

    use crate::ext_vec::ByteVec;

    #[test]
    fn insert() {
        let mut s = vec![];
        s.insert_str(0, "foo");
        assert_eq!(s, "foo".as_bytes());

        let mut s = Vec::from("a");
        s.insert_str(0, "foo");
        assert_eq!(s, "fooa".as_bytes());

        let mut s = Vec::from("a");
        s.insert_str(1, "foo");
        assert_eq!(s, "afoo".as_bytes());

        let mut s = Vec::from("foobar");
        s.insert_str(3, "quux");
        assert_eq!(s, "fooquuxbar".as_bytes());

        let mut s = Vec::from("foobar");
        s.insert_str(3, "x");
        assert_eq!(s, "fooxbar".as_bytes());

        let mut s = Vec::from("foobar");
        s.insert_str(0, "x");
        assert_eq!(s, "xfoobar".as_bytes());

        let mut s = Vec::from("foobar");
        s.insert_str(6, "x");
        assert_eq!(s, "foobarx".as_bytes());

        let mut s = Vec::from("foobar");
        s.insert_str(3, "quuxbazquux");
        assert_eq!(s, "fooquuxbazquuxbar".as_bytes());
    }

    #[test]
    #[should_panic]
    fn insert_fail1() {
        let mut s = vec![];
        s.insert_str(1, "foo");
    }

    #[test]
    #[should_panic]
    fn insert_fail2() {
        let mut s = Vec::from("a");
        s.insert_str(2, "foo");
    }

    #[test]
    #[should_panic]
    fn insert_fail3() {
        let mut s = Vec::from("foobar");
        s.insert_str(7, "foo");
    }
}
