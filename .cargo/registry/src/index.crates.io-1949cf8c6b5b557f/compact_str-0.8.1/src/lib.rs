#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

#[cfg_attr(test, macro_use)]
extern crate alloc;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
#[doc(hidden)]
pub use core;
use core::borrow::{
    Borrow,
    BorrowMut,
};
use core::cmp::Ordering;
use core::hash::{
    Hash,
    Hasher,
};
use core::iter::FusedIterator;
use core::ops::{
    Add,
    AddAssign,
    Bound,
    Deref,
    DerefMut,
    RangeBounds,
};
use core::str::{
    FromStr,
    Utf8Error,
};
use core::{
    fmt,
    mem,
    slice,
};
#[cfg(feature = "std")]
use std::ffi::OsStr;

mod features;
mod macros;
mod unicode_data;

mod repr;
use repr::Repr;

mod traits;
pub use traits::{
    CompactStringExt,
    ToCompactString,
};

#[cfg(test)]
mod tests;

/// A [`CompactString`] is a compact string type that can be used almost anywhere a
/// [`String`] or [`str`] can be used.
///
/// ## Using `CompactString`
/// ```
/// use compact_str::CompactString;
/// # use std::collections::HashMap;
///
/// // CompactString auto derefs into a str so you can use all methods from `str`
/// // that take a `&self`
/// if CompactString::new("hello world!").is_ascii() {
///     println!("we're all ASCII")
/// }
///
/// // You can use a CompactString in collections like you would a String or &str
/// let mut map: HashMap<CompactString, CompactString> = HashMap::new();
///
/// // directly construct a new `CompactString`
/// map.insert(CompactString::new("nyc"), CompactString::new("empire state building"));
/// // create a `CompactString` from a `&str`
/// map.insert("sf".into(), "transamerica pyramid".into());
/// // create a `CompactString` from a `String`
/// map.insert(String::from("sea").into(), String::from("space needle").into());
///
/// fn wrapped_print<T: AsRef<str>>(text: T) {
///     println!("{}", text.as_ref());
/// }
///
/// // CompactString impls AsRef<str> and Borrow<str>, so it can be used anywhere
/// // that expects a generic string
/// if let Some(building) = map.get("nyc") {
///     wrapped_print(building);
/// }
///
/// // CompactString can also be directly compared to a String or &str
/// assert_eq!(CompactString::new("chicago"), "chicago");
/// assert_eq!(CompactString::new("houston"), String::from("houston"));
/// ```
///
/// # Converting from a `String`
/// It's important that a `CompactString` interops well with `String`, so you can easily use both in
/// your code base.
///
/// `CompactString` implements `From<String>` and operates in the following manner:
/// - Eagerly inlines the string, possibly dropping excess capacity
/// - Otherwise re-uses the same underlying buffer from `String`
///
/// ```
/// use compact_str::CompactString;
///
/// // eagerly inlining
/// let short = String::from("hello world");
/// let short_c = CompactString::from(short);
/// assert!(!short_c.is_heap_allocated());
///
/// // dropping excess capacity
/// let mut excess = String::with_capacity(256);
/// excess.push_str("abc");
///
/// let excess_c = CompactString::from(excess);
/// assert!(!excess_c.is_heap_allocated());
/// assert!(excess_c.capacity() < 256);
///
/// // re-using the same buffer
/// let long = String::from("this is a longer string that will be heap allocated");
///
/// let long_ptr = long.as_ptr();
/// let long_len = long.len();
/// let long_cap = long.capacity();
///
/// let mut long_c = CompactString::from(long);
/// assert!(long_c.is_heap_allocated());
///
/// let cpt_ptr = long_c.as_ptr();
/// let cpt_len = long_c.len();
/// let cpt_cap = long_c.capacity();
///
/// // the original String and the CompactString point to the same place in memory, buffer re-use!
/// assert_eq!(cpt_ptr, long_ptr);
/// assert_eq!(cpt_len, long_len);
/// assert_eq!(cpt_cap, long_cap);
/// ```
///
/// ### Prevent Eagerly Inlining
/// A consequence of eagerly inlining is you then need to de-allocate the existing buffer, which
/// might not always be desirable if you're converting a very large amount of `String`s. If your
/// code is very sensitive to allocations, consider the [`CompactString::from_string_buffer`] API.
#[repr(transparent)]
pub struct CompactString(Repr);

impl CompactString {
    /// Creates a new [`CompactString`] from any type that implements `AsRef<str>`.
    /// If the string is short enough, then it will be inlined on the stack!
    ///
    /// In a `static` or `const` context you can use the method [`CompactString::const_new()`].
    ///
    /// # Examples
    ///
    /// ### Inlined
    /// ```
    /// # use compact_str::CompactString;
    /// // We can inline strings up to 12 characters long on 32-bit architectures...
    /// #[cfg(target_pointer_width = "32")]
    /// let s = "i'm 12 chars";
    /// // ...and up to 24 characters on 64-bit architectures!
    /// #[cfg(target_pointer_width = "64")]
    /// let s = "i am 24 characters long!";
    ///
    /// let compact = CompactString::new(&s);
    ///
    /// assert_eq!(compact, s);
    /// // we are not allocated on the heap!
    /// assert!(!compact.is_heap_allocated());
    /// ```
    ///
    /// ### Heap
    /// ```
    /// # use compact_str::CompactString;
    /// // For longer strings though, we get allocated on the heap
    /// let long = "I am a longer string that will be allocated on the heap";
    /// let compact = CompactString::new(long);
    ///
    /// assert_eq!(compact, long);
    /// // we are allocated on the heap!
    /// assert!(compact.is_heap_allocated());
    /// ```
    ///
    /// ### Creation
    /// ```
    /// use compact_str::CompactString;
    ///
    /// // Using a `&'static str`
    /// let s = "hello world!";
    /// let hello = CompactString::new(&s);
    ///
    /// // Using a `String`
    /// let u = String::from("🦄🌈");
    /// let unicorn = CompactString::new(u);
    ///
    /// // Using a `Box<str>`
    /// let b: Box<str> = String::from("📦📦📦").into_boxed_str();
    /// let boxed = CompactString::new(&b);
    /// ```
    #[inline]
    #[track_caller]
    pub fn new<T: AsRef<str>>(text: T) -> Self {
        Self::try_new(text).unwrap_with_msg()
    }

    /// Fallible version of [`CompactString::new()`]
    ///
    /// This method won't panic if the system is out-of-memory, but return an [`ReserveError`].
    /// Otherwise it behaves the same as [`CompactString::new()`].
    #[inline]
    pub fn try_new<T: AsRef<str>>(text: T) -> Result<Self, ReserveError> {
        Repr::new(text.as_ref()).map(CompactString)
    }

    /// Creates a new inline [`CompactString`] from `&'static str` at compile time.
    /// Complexity: O(1). As an optimization, short strings get inlined.
    ///
    /// In a dynamic context you can use the method [`CompactString::new()`].
    ///
    /// # Examples
    /// ```
    /// use compact_str::CompactString;
    ///
    /// const DEFAULT_NAME: CompactString = CompactString::const_new("untitled");
    /// ```
    #[inline]
    pub const fn const_new(text: &'static str) -> Self {
        CompactString(Repr::const_new(text))
    }

    /// Creates a new inline [`CompactString`] at compile time.
    #[deprecated(
        since = "0.8.0",
        note = "replaced by CompactString::const_new, will be removed in 0.9.0"
    )]
    #[inline]
    pub const fn new_inline(text: &'static str) -> Self {
        CompactString::const_new(text)
    }

    /// Creates a new inline [`CompactString`] from `&'static str` at compile time.
    #[deprecated(
        since = "0.8.0",
        note = "replaced by CompactString::const_new, will be removed in 0.9.0"
    )]
    #[inline]
    pub const fn from_static_str(text: &'static str) -> Self {
        CompactString::const_new(text)
    }

    /// Get back the `&'static str` constructed by [`CompactString::const_new`].
    ///
    /// If the string was short enough that it could be inlined, then it was inline, and
    /// this method will return `None`.
    ///
    /// # Examples
    /// ```
    /// use compact_str::CompactString;
    ///
    /// const DEFAULT_NAME: CompactString =
    ///     CompactString::const_new("That is not dead which can eternal lie.");
    /// assert_eq!(
    ///     DEFAULT_NAME.as_static_str().unwrap(),
    ///     "That is not dead which can eternal lie.",
    /// );
    /// ```
    #[inline]
    #[rustversion::attr(since(1.64), const)]
    pub fn as_static_str(&self) -> Option<&'static str> {
        self.0.as_static_str()
    }

    /// Creates a new empty [`CompactString`] with the capacity to fit at least `capacity` bytes.
    ///
    /// A `CompactString` will inline strings on the stack, if they're small enough. Specifically,
    /// if the string has a length less than or equal to `std::mem::size_of::<String>` bytes
    /// then it will be inlined. This also means that `CompactString`s have a minimum capacity
    /// of `std::mem::size_of::<String>`.
    ///
    /// # Panics
    ///
    /// This method panics if the system is out-of-memory.
    /// Use [`CompactString::try_with_capacity()`] if you want to handle such a problem manually.
    ///
    /// # Examples
    ///
    /// ### "zero" Capacity
    /// ```
    /// # use compact_str::CompactString;
    /// // Creating a CompactString with a capacity of 0 will create
    /// // one with capacity of std::mem::size_of::<String>();
    /// let empty = CompactString::with_capacity(0);
    /// let min_size = std::mem::size_of::<String>();
    ///
    /// assert_eq!(empty.capacity(), min_size);
    /// assert_ne!(0, min_size);
    /// assert!(!empty.is_heap_allocated());
    /// ```
    ///
    /// ### Max Inline Size
    /// ```
    /// # use compact_str::CompactString;
    /// // Creating a CompactString with a capacity of std::mem::size_of::<String>()
    /// // will not heap allocate.
    /// let str_size = std::mem::size_of::<String>();
    /// let empty = CompactString::with_capacity(str_size);
    ///
    /// assert_eq!(empty.capacity(), str_size);
    /// assert!(!empty.is_heap_allocated());
    /// ```
    ///
    /// ### Heap Allocating
    /// ```
    /// # use compact_str::CompactString;
    /// // If you create a `CompactString` with a capacity greater than
    /// // `std::mem::size_of::<String>`, it will heap allocated. For heap
    /// // allocated strings we have a minimum capacity
    ///
    /// const MIN_HEAP_CAPACITY: usize = std::mem::size_of::<usize>() * 4;
    ///
    /// let heap_size = std::mem::size_of::<String>() + 1;
    /// let empty = CompactString::with_capacity(heap_size);
    ///
    /// assert_eq!(empty.capacity(), MIN_HEAP_CAPACITY);
    /// assert!(empty.is_heap_allocated());
    /// ```
    #[inline]
    #[track_caller]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::try_with_capacity(capacity).unwrap_with_msg()
    }

    /// Fallible version of [`CompactString::with_capacity()`]
    ///
    /// This method won't panic if the system is out-of-memory, but return an [`ReserveError`].
    /// Otherwise it behaves the same as [`CompactString::with_capacity()`].
    #[inline]
    pub fn try_with_capacity(capacity: usize) -> Result<Self, ReserveError> {
        Repr::with_capacity(capacity).map(CompactString)
    }

    /// Convert a slice of bytes into a [`CompactString`].
    ///
    /// A [`CompactString`] is a contiguous collection of bytes (`u8`s) that is valid [`UTF-8`](https://en.wikipedia.org/wiki/UTF-8).
    /// This method converts from an arbitrary contiguous collection of bytes into a
    /// [`CompactString`], failing if the provided bytes are not `UTF-8`.
    ///
    /// Note: If you want to create a [`CompactString`] from a non-contiguous collection of bytes,
    /// enable the `bytes` feature of this crate, and see `CompactString::from_utf8_buf`
    ///
    /// # Examples
    /// ### Valid UTF-8
    /// ```
    /// # use compact_str::CompactString;
    /// let bytes = vec![240, 159, 166, 128, 240, 159, 146, 175];
    /// let compact = CompactString::from_utf8(bytes).expect("valid UTF-8");
    ///
    /// assert_eq!(compact, "🦀💯");
    /// ```
    ///
    /// ### Invalid UTF-8
    /// ```
    /// # use compact_str::CompactString;
    /// let bytes = vec![255, 255, 255];
    /// let result = CompactString::from_utf8(bytes);
    ///
    /// assert!(result.is_err());
    /// ```
    #[inline]
    pub fn from_utf8<B: AsRef<[u8]>>(buf: B) -> Result<Self, Utf8Error> {
        Repr::from_utf8(buf).map(CompactString)
    }

    /// Converts a vector of bytes to a [`CompactString`] without checking that the string contains
    /// valid UTF-8.
    ///
    /// See the safe version, [`CompactString::from_utf8`], for more details.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not check that the bytes passed to it are valid
    /// UTF-8. If this constraint is violated, it may cause memory unsafety issues with future users
    /// of the [`CompactString`], as the rest of the standard library assumes that
    /// [`CompactString`]s are valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// // some bytes, in a vector
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// let sparkle_heart = unsafe {
    ///     CompactString::from_utf8_unchecked(sparkle_heart)
    /// };
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub unsafe fn from_utf8_unchecked<B: AsRef<[u8]>>(buf: B) -> Self {
        Repr::from_utf8_unchecked(buf)
            .map(CompactString)
            .unwrap_with_msg()
    }

    /// Decode a [`UTF-16`](https://en.wikipedia.org/wiki/UTF-16) slice of bytes into a
    /// [`CompactString`], returning an [`Err`] if the slice contains any invalid data.
    ///
    /// # Examples
    /// ### Valid UTF-16
    /// ```
    /// # use compact_str::CompactString;
    /// let buf: &[u16] = &[0xD834, 0xDD1E, 0x006d, 0x0075, 0x0073, 0x0069, 0x0063];
    /// let compact = CompactString::from_utf16(buf).unwrap();
    ///
    /// assert_eq!(compact, "𝄞music");
    /// ```
    ///
    /// ### Invalid UTF-16
    /// ```
    /// # use compact_str::CompactString;
    /// let buf: &[u16] = &[0xD834, 0xDD1E, 0x006d, 0x0075, 0xD800, 0x0069, 0x0063];
    /// let res = CompactString::from_utf16(buf);
    ///
    /// assert!(res.is_err());
    /// ```
    #[inline]
    pub fn from_utf16<B: AsRef<[u16]>>(buf: B) -> Result<Self, Utf16Error> {
        // Note: we don't use collect::<Result<_, _>>() because that fails to pre-allocate a buffer,
        // even though the size of our iterator, `buf`, is known ahead of time.
        //
        // rustlang issue #48994 is tracking the fix

        let buf = buf.as_ref();
        let mut ret = CompactString::with_capacity(buf.len());
        for c in core::char::decode_utf16(buf.iter().copied()) {
            if let Ok(c) = c {
                ret.push(c);
            } else {
                return Err(Utf16Error(()));
            }
        }
        Ok(ret)
    }

    /// Decode a UTF-16–encoded slice `v` into a `CompactString`, replacing invalid data with
    /// the replacement character (`U+FFFD`), �.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// // 𝄞mus<invalid>ic<invalid>
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
    ///           0x0073, 0xDD1E, 0x0069, 0x0063,
    ///           0xD834];
    ///
    /// assert_eq!(CompactString::from("𝄞mus\u{FFFD}ic\u{FFFD}"),
    ///            CompactString::from_utf16_lossy(v));
    /// ```
    #[inline]
    pub fn from_utf16_lossy<B: AsRef<[u16]>>(buf: B) -> Self {
        let buf = buf.as_ref();
        let mut ret = CompactString::with_capacity(buf.len());
        for c in core::char::decode_utf16(buf.iter().copied()) {
            match c {
                Ok(c) => ret.push(c),
                Err(_) => ret.push_str("�"),
            }
        }
        ret
    }

    /// Returns the length of the [`CompactString`] in `bytes`, not [`char`]s or graphemes.
    ///
    /// When using `UTF-8` encoding (which all strings in Rust do) a single character will be 1 to 4
    /// bytes long, therefore the return value of this method might not be what a human considers
    /// the length of the string.
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let ascii = CompactString::new("hello world");
    /// assert_eq!(ascii.len(), 11);
    ///
    /// let emoji = CompactString::new("👱");
    /// assert_eq!(emoji.len(), 4);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the [`CompactString`] has a length of 0, `false` otherwise
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut msg = CompactString::new("");
    /// assert!(msg.is_empty());
    ///
    /// // add some characters
    /// msg.push_str("hello reader!");
    /// assert!(!msg.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the capacity of the [`CompactString`], in bytes.
    ///
    /// # Note
    /// * A `CompactString` will always have a capacity of at least `std::mem::size_of::<String>()`
    ///
    /// # Examples
    /// ### Minimum Size
    /// ```
    /// # use compact_str::CompactString;
    /// let min_size = std::mem::size_of::<String>();
    /// let compact = CompactString::new("");
    ///
    /// assert!(compact.capacity() >= min_size);
    /// ```
    ///
    /// ### Heap Allocated
    /// ```
    /// # use compact_str::CompactString;
    /// let compact = CompactString::with_capacity(128);
    /// assert_eq!(compact.capacity(), 128);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Ensures that this [`CompactString`]'s capacity is at least `additional` bytes longer than
    /// its length. The capacity may be increased by more than `additional` bytes if it chooses,
    /// to prevent frequent reallocations.
    ///
    /// # Note
    /// * A `CompactString` will always have at least a capacity of `std::mem::size_of::<String>()`
    /// * Reserving additional bytes may cause the `CompactString` to become heap allocated
    ///
    /// # Panics
    /// This method panics if the new capacity overflows `usize` or if the system is out-of-memory.
    /// Use [`CompactString::try_reserve()`] if you want to handle such a problem manually.
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    ///
    /// const WORD: usize = std::mem::size_of::<usize>();
    /// let mut compact = CompactString::default();
    /// assert!(compact.capacity() >= (WORD * 3) - 1);
    ///
    /// compact.reserve(200);
    /// assert!(compact.is_heap_allocated());
    /// assert!(compact.capacity() >= 200);
    /// ```
    #[inline]
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        self.try_reserve(additional).unwrap_with_msg()
    }

    /// Fallible version of [`CompactString::reserve()`]
    ///
    /// This method won't panic if the system is out-of-memory, but return an [`ReserveError`]
    /// Otherwise it behaves the same as [`CompactString::reserve()`].
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), ReserveError> {
        self.0.reserve(additional)
    }

    /// Returns a string slice containing the entire [`CompactString`].
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let s = CompactString::new("hello");
    ///
    /// assert_eq!(s.as_str(), "hello");
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns a mutable string slice containing the entire [`CompactString`].
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("hello");
    /// s.as_mut_str().make_ascii_uppercase();
    ///
    /// assert_eq!(s.as_str(), "HELLO");
    /// ```
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        let len = self.len();
        unsafe { core::str::from_utf8_unchecked_mut(&mut self.0.as_mut_buf()[..len]) }
    }

    unsafe fn spare_capacity_mut(&mut self) -> &mut [mem::MaybeUninit<u8>] {
        let buf = self.0.as_mut_buf();
        let ptr = buf.as_mut_ptr();
        let cap = buf.len();
        let len = self.len();

        slice::from_raw_parts_mut(ptr.add(len) as *mut mem::MaybeUninit<u8>, cap - len)
    }

    /// Returns a byte slice of the [`CompactString`]'s contents.
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let s = CompactString::new("hello");
    ///
    /// assert_eq!(&[104, 101, 108, 108, 111], s.as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_slice()[..self.len()]
    }

    // TODO: Implement a `try_as_mut_slice(...)` that will fail if it results in cloning?
    //
    /// Provides a mutable reference to the underlying buffer of bytes.
    ///
    /// # Safety
    /// * All Rust strings, including `CompactString`, must be valid UTF-8. The caller must
    ///   guarantee that any modifications made to the underlying buffer are valid UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("hello");
    ///
    /// let slice = unsafe { s.as_mut_bytes() };
    /// // copy bytes into our string
    /// slice[5..11].copy_from_slice(" world".as_bytes());
    /// // set the len of the string
    /// unsafe { s.set_len(11) };
    ///
    /// assert_eq!(s, "hello world");
    /// ```
    #[inline]
    pub unsafe fn as_mut_bytes(&mut self) -> &mut [u8] {
        self.0.as_mut_buf()
    }

    /// Appends the given [`char`] to the end of this [`CompactString`].
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("foo");
    ///
    /// s.push('b');
    /// s.push('a');
    /// s.push('r');
    ///
    /// assert_eq!("foobar", s);
    /// ```
    pub fn push(&mut self, ch: char) {
        self.push_str(ch.encode_utf8(&mut [0; 4]));
    }

    /// Removes the last character from the [`CompactString`] and returns it.
    /// Returns `None` if this [`CompactString`] is empty.
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("abc");
    ///
    /// assert_eq!(s.pop(), Some('c'));
    /// assert_eq!(s.pop(), Some('b'));
    /// assert_eq!(s.pop(), Some('a'));
    ///
    /// assert_eq!(s.pop(), None);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        self.0.pop()
    }

    /// Appends a given string slice onto the end of this [`CompactString`]
    ///
    /// # Examples
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("abc");
    ///
    /// s.push_str("123");
    ///
    /// assert_eq!("abc123", s);
    /// ```
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.0.push_str(s)
    }

    /// Removes a [`char`] from this [`CompactString`] at a byte position and returns it.
    ///
    /// This is an *O*(*n*) operation, as it requires copying every element in the
    /// buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than or equal to the [`CompactString`]'s length,
    /// or if it does not lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// ### Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut c = CompactString::from("hello world");
    ///
    /// assert_eq!(c.remove(0), 'h');
    /// assert_eq!(c, "ello world");
    ///
    /// assert_eq!(c.remove(5), 'w');
    /// assert_eq!(c, "ello orld");
    /// ```
    ///
    /// ### Past total length:
    ///
    /// ```should_panic
    /// # use compact_str::CompactString;
    /// let mut c = CompactString::from("hello there!");
    /// c.remove(100);
    /// ```
    ///
    /// ### Not on char boundary:
    ///
    /// ```should_panic
    /// # use compact_str::CompactString;
    /// let mut c = CompactString::from("🦄");
    /// c.remove(1);
    /// ```
    #[inline]
    pub fn remove(&mut self, idx: usize) -> char {
        let len = self.len();
        let substr = &mut self.as_mut_str()[idx..];

        // get the char we want to remove
        let ch = substr
            .chars()
            .next()
            .expect("cannot remove a char from the end of a string");
        let ch_len = ch.len_utf8();

        // shift everything back one character
        let num_bytes = substr.len() - ch_len;
        let ptr = substr.as_mut_ptr();

        // SAFETY: Both src and dest are valid for reads of `num_bytes` amount of bytes,
        // and are properly aligned
        unsafe {
            core::ptr::copy(ptr.add(ch_len) as *const u8, ptr, num_bytes);
            self.set_len(len - ch_len);
        }

        ch
    }

    /// Forces the length of the [`CompactString`] to `new_len`.
    ///
    /// This is a low-level operation that maintains none of the normal invariants for
    /// `CompactString`. If you want to modify the `CompactString` you should use methods like
    /// `push`, `push_str` or `pop`.
    ///
    /// # Safety
    /// * `new_len` must be less than or equal to `capacity()`
    /// * The elements at `old_len..new_len` must be initialized
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.0.set_len(new_len)
    }

    /// Returns whether or not the [`CompactString`] is heap allocated.
    ///
    /// # Examples
    /// ### Inlined
    /// ```
    /// # use compact_str::CompactString;
    /// let hello = CompactString::new("hello world");
    ///
    /// assert!(!hello.is_heap_allocated());
    /// ```
    ///
    /// ### Heap Allocated
    /// ```
    /// # use compact_str::CompactString;
    /// let msg = CompactString::new("this message will self destruct in 5, 4, 3, 2, 1 💥");
    ///
    /// assert!(msg.is_heap_allocated());
    /// ```
    #[inline]
    pub fn is_heap_allocated(&self) -> bool {
        self.0.is_heap_allocated()
    }

    /// Ensure that the given range is inside the set data, and that no codepoints are split.
    ///
    /// Returns the range `start..end` as a tuple.
    #[inline]
    fn ensure_range(&self, range: impl RangeBounds<usize>) -> (usize, usize) {
        #[cold]
        #[inline(never)]
        fn illegal_range() -> ! {
            panic!("illegal range");
        }

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => match n.checked_add(1) {
                Some(n) => n,
                None => illegal_range(),
            },
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => match n.checked_add(1) {
                Some(n) => n,
                None => illegal_range(),
            },
            Bound::Excluded(&n) => n,
            Bound::Unbounded => self.len(),
        };
        if end < start {
            illegal_range();
        }

        let s = self.as_str();
        if !s.is_char_boundary(start) || !s.is_char_boundary(end) {
            illegal_range();
        }

        (start, end)
    }

    /// Removes the specified range in the [`CompactString`],
    /// and replaces it with the given string.
    /// The given string doesn't need to be the same length as the range.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Hello, world!");
    ///
    /// s.replace_range(7..12, "WORLD");
    /// assert_eq!(s, "Hello, WORLD!");
    ///
    /// s.replace_range(7..=11, "you");
    /// assert_eq!(s, "Hello, you!");
    ///
    /// s.replace_range(5.., "! Is it me you're looking for?");
    /// assert_eq!(s, "Hello! Is it me you're looking for?");
    /// ```
    #[inline]
    pub fn replace_range(&mut self, range: impl RangeBounds<usize>, replace_with: &str) {
        let (start, end) = self.ensure_range(range);
        let dest_len = end - start;
        match dest_len.cmp(&replace_with.len()) {
            Ordering::Equal => unsafe { self.replace_range_same_size(start, end, replace_with) },
            Ordering::Greater => unsafe { self.replace_range_shrink(start, end, replace_with) },
            Ordering::Less => unsafe { self.replace_range_grow(start, end, replace_with) },
        }
    }

    /// Replace into the same size.
    unsafe fn replace_range_same_size(&mut self, start: usize, end: usize, replace_with: &str) {
        core::ptr::copy_nonoverlapping(
            replace_with.as_ptr(),
            self.as_mut_ptr().add(start),
            end - start,
        );
    }

    /// Replace, so self.len() gets smaller.
    unsafe fn replace_range_shrink(&mut self, start: usize, end: usize, replace_with: &str) {
        let total_len = self.len();
        let dest_len = end - start;
        let new_len = total_len - (dest_len - replace_with.len());
        let amount = total_len - end;
        let data = self.as_mut_ptr();
        // first insert the replacement string, overwriting the current content
        core::ptr::copy_nonoverlapping(replace_with.as_ptr(), data.add(start), replace_with.len());
        // then move the tail of the CompactString forward to its new place, filling the gap
        core::ptr::copy(
            data.add(total_len - amount),
            data.add(new_len - amount),
            amount,
        );
        // and lastly we set the new length
        self.set_len(new_len);
    }

    /// Replace, so self.len() gets bigger.
    unsafe fn replace_range_grow(&mut self, start: usize, end: usize, replace_with: &str) {
        let dest_len = end - start;
        self.reserve(replace_with.len() - dest_len);
        let total_len = self.len();
        let new_len = total_len + (replace_with.len() - dest_len);
        let amount = total_len - end;
        // first grow the string, so MIRI knows that the full range is usable
        self.set_len(new_len);
        let data = self.as_mut_ptr();
        // then move the tail of the CompactString back to its new place
        core::ptr::copy(
            data.add(total_len - amount),
            data.add(new_len - amount),
            amount,
        );
        // and lastly insert the replacement string
        core::ptr::copy_nonoverlapping(replace_with.as_ptr(), data.add(start), replace_with.len());
    }

    /// Creates a new [`CompactString`] by repeating a string `n` times.
    ///
    /// # Panics
    ///
    /// This function will panic if the capacity would overflow.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// assert_eq!(CompactString::new("abc").repeat(4), CompactString::new("abcabcabcabc"));
    /// ```
    ///
    /// A panic upon overflow:
    ///
    /// ```should_panic
    /// use compact_str::CompactString;
    ///
    /// // this will panic at runtime
    /// let huge = CompactString::new("0123456789abcdef").repeat(usize::MAX);
    /// ```
    #[must_use]
    pub fn repeat(&self, n: usize) -> Self {
        if n == 0 || self.is_empty() {
            Self::const_new("")
        } else if n == 1 {
            self.clone()
        } else {
            let mut out = Self::with_capacity(self.len() * n);
            (0..n).for_each(|_| out.push_str(self));
            out
        }
    }

    /// Truncate the [`CompactString`] to a shorter length.
    ///
    /// If the length of the [`CompactString`] is less or equal to `new_len`, the call is a no-op.
    ///
    /// Calling this function does not change the capacity of the [`CompactString`].
    ///
    /// # Panics
    ///
    /// Panics if the new end of the string does not lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Hello, world!");
    /// s.truncate(5);
    /// assert_eq!(s, "Hello");
    /// ```
    pub fn truncate(&mut self, new_len: usize) {
        let s = self.as_str();
        if new_len >= s.len() {
            return;
        }

        assert!(
            s.is_char_boundary(new_len),
            "new_len must lie on char boundary",
        );
        unsafe { self.set_len(new_len) };
    }

    /// Converts a [`CompactString`] to a raw pointer.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_slice().as_ptr()
    }

    /// Converts a mutable [`CompactString`] to a raw pointer.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        unsafe { self.0.as_mut_buf().as_mut_ptr() }
    }

    /// Insert string character at an index.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Hello!");
    /// s.insert_str(5, ", world");
    /// assert_eq!(s, "Hello, world!");
    /// ```
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        assert!(self.is_char_boundary(idx), "idx must lie on char boundary");

        let new_len = self.len() + string.len();
        self.reserve(string.len());

        // SAFETY: We just checked that we may split self at idx.
        //         We set the length only after reserving the memory.
        //         We fill the gap with valid UTF-8 data.
        unsafe {
            // first move the tail to the new back
            let data = self.as_mut_ptr();
            core::ptr::copy(
                data.add(idx),
                data.add(idx + string.len()),
                new_len - idx - string.len(),
            );

            // then insert the new bytes
            core::ptr::copy_nonoverlapping(string.as_ptr(), data.add(idx), string.len());

            // and lastly resize the string
            self.set_len(new_len);
        }
    }

    /// Insert a character at an index.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Hello world!");
    /// s.insert(5, ',');
    /// assert_eq!(s, "Hello, world!");
    /// ```
    pub fn insert(&mut self, idx: usize, ch: char) {
        self.insert_str(idx, ch.encode_utf8(&mut [0; 4]));
    }

    /// Reduces the length of the [`CompactString`] to zero.
    ///
    /// Calling this function does not change the capacity of the [`CompactString`].
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Rust is the most loved language on Stackoverflow!");
    /// assert_eq!(s.capacity(), 49);
    ///
    /// s.clear();
    ///
    /// assert_eq!(s, "");
    /// assert_eq!(s.capacity(), 49);
    /// ```
    pub fn clear(&mut self) {
        unsafe { self.set_len(0) };
    }

    /// Split the [`CompactString`] into at the given byte index.
    ///
    /// Calling this function does not change the capacity of the [`CompactString`], unless the
    /// [`CompactString`] is backed by a `&'static str`.
    ///
    /// # Panics
    ///
    /// Panics if `at` does not lie on a [`char`] boundary.
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::const_new("Hello, world!");
    /// let w = s.split_off(5);
    ///
    /// assert_eq!(w, ", world!");
    /// assert_eq!(s, "Hello");
    /// ```
    pub fn split_off(&mut self, at: usize) -> Self {
        if let Some(s) = self.as_static_str() {
            let result = Self::const_new(&s[at..]);
            // SAFETY: the previous line `self[at...]` would have panicked if `at` was invalid
            unsafe { self.set_len(at) };
            result
        } else {
            let result = self[at..].into();
            // SAFETY: the previous line `self[at...]` would have panicked if `at` was invalid
            unsafe { self.set_len(at) };
            result
        }
    }

    /// Remove a range from the [`CompactString`], and return it as an iterator.
    ///
    /// Calling this function does not change the capacity of the [`CompactString`].
    ///
    /// # Panics
    ///
    /// Panics if the start or end of the range does not lie on a [`char`] boundary.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::new("Hello, world!");
    ///
    /// let mut d = s.drain(5..12);
    /// assert_eq!(d.next(), Some(','));   // iterate over the extracted data
    /// assert_eq!(d.as_str(), " world"); // or get the whole data as &str
    ///
    /// // The iterator keeps a reference to `s`, so you have to drop() the iterator,
    /// // before you can access `s` again.
    /// drop(d);
    /// assert_eq!(s, "Hello!");
    /// ```
    pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<'_> {
        let (start, end) = self.ensure_range(range);
        Drain {
            compact_string: self as *mut Self,
            start,
            end,
            chars: self[start..end].chars(),
        }
    }

    /// Shrinks the capacity of this [`CompactString`] with a lower bound.
    ///
    /// The resulting capactity is never less than the size of 3×[`usize`],
    /// i.e. the capacity than can be inlined.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::with_capacity(100);
    /// assert_eq!(s.capacity(), 100);
    ///
    /// // if the capacity was already bigger than the argument, the call is a no-op
    /// s.shrink_to(100);
    /// assert_eq!(s.capacity(), 100);
    ///
    /// s.shrink_to(50);
    /// assert_eq!(s.capacity(), 50);
    ///
    /// // if the string can be inlined, it is
    /// s.shrink_to(10);
    /// assert_eq!(s.capacity(), 3 * std::mem::size_of::<usize>());
    /// ```
    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }

    /// Shrinks the capacity of this [`CompactString`] to match its length.
    ///
    /// The resulting capactity is never less than the size of 3×[`usize`],
    /// i.e. the capacity than can be inlined.
    ///
    /// This method is effectively the same as calling [`string.shrink_to(0)`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::from("This is a string with more than 24 characters.");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    ///  s.shrink_to_fit();
    /// assert_eq!(s.len(), s.capacity());
    /// ```
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::from("short string");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to_fit();
    /// assert_eq!(s.capacity(), 3 * std::mem::size_of::<usize>());
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to(0);
    }

    /// Retains only the characters specified by the predicate.
    ///
    /// The method iterates over the characters in the string and calls the `predicate`.
    ///
    /// If the `predicate` returns `false`, then the character gets removed.
    /// If the `predicate` returns `true`, then the character is kept.
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let mut s = CompactString::from("äb𝄞d€");
    ///
    /// let keep = [false, true, true, false, true];
    /// let mut iter = keep.iter();
    /// s.retain(|_| *iter.next().unwrap());
    ///
    /// assert_eq!(s, "b𝄞€");
    /// ```
    pub fn retain(&mut self, mut predicate: impl FnMut(char) -> bool) {
        // We iterate over the string, and copy character by character.

        struct SetLenOnDrop<'a> {
            self_: &'a mut CompactString,
            src_idx: usize,
            dst_idx: usize,
        }

        let mut g = SetLenOnDrop {
            self_: self,
            src_idx: 0,
            dst_idx: 0,
        };
        let s = g.self_.as_mut_str();
        while let Some(ch) = s[g.src_idx..].chars().next() {
            let ch_len = ch.len_utf8();
            if predicate(ch) {
                // SAFETY: We know that both indices are valid, and that we don't split a char.
                unsafe {
                    let p = s.as_mut_ptr();
                    core::ptr::copy(p.add(g.src_idx), p.add(g.dst_idx), ch_len);
                }
                g.dst_idx += ch_len;
            }
            g.src_idx += ch_len;
        }

        impl Drop for SetLenOnDrop<'_> {
            fn drop(&mut self) {
                // SAFETY: We know that the index is a valid position to break the string.
                unsafe { self.self_.set_len(self.dst_idx) };
            }
        }
        drop(g);
    }

    /// Decode a bytes slice as UTF-8 string, replacing any illegal codepoints
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let chess_knight = b"\xf0\x9f\xa8\x84";
    ///
    /// assert_eq!(
    ///     "🨄",
    ///     CompactString::from_utf8_lossy(chess_knight),
    /// );
    ///
    /// // For valid UTF-8 slices, this is the same as:
    /// assert_eq!(
    ///     "🨄",
    ///     CompactString::new(std::str::from_utf8(chess_knight).unwrap()),
    /// );
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let broken = b"\xf0\x9f\xc8\x84";
    ///
    /// assert_eq!(
    ///     "�Ȅ",
    ///     CompactString::from_utf8_lossy(broken),
    /// );
    ///
    /// // For invalid UTF-8 slices, this is an optimized implemented for:
    /// assert_eq!(
    ///     "�Ȅ",
    ///     CompactString::from(String::from_utf8_lossy(broken)),
    /// );
    /// ```
    pub fn from_utf8_lossy(v: &[u8]) -> Self {
        fn next_char<'a>(
            iter: &mut <&[u8] as IntoIterator>::IntoIter,
            buf: &'a mut [u8; 4],
        ) -> Option<&'a [u8]> {
            const REPLACEMENT: &[u8] = "\u{FFFD}".as_bytes();

            macro_rules! ensure_range {
                ($idx:literal, $range:pat) => {{
                    let mut i = iter.clone();
                    match i.next() {
                        Some(&c) if matches!(c, $range) => {
                            buf[$idx] = c;
                            *iter = i;
                        }
                        _ => return Some(REPLACEMENT),
                    }
                }};
            }

            macro_rules! ensure_cont {
                ($idx:literal) => {{
                    ensure_range!($idx, 0x80..=0xBF);
                }};
            }

            let c = *iter.next()?;
            buf[0] = c;

            match c {
                0x00..=0x7F => {
                    // simple ASCII: push as is
                    Some(&buf[..1])
                }
                0xC2..=0xDF => {
                    // two bytes
                    ensure_cont!(1);
                    Some(&buf[..2])
                }
                0xE0..=0xEF => {
                    // three bytes
                    match c {
                        // 0x80..=0x9F encodes surrogate half
                        0xE0 => ensure_range!(1, 0xA0..=0xBF),
                        // 0xA0..=0xBF encodes surrogate half
                        0xED => ensure_range!(1, 0x80..=0x9F),
                        // all UTF-8 continuation bytes are valid
                        _ => ensure_cont!(1),
                    }
                    ensure_cont!(2);
                    Some(&buf[..3])
                }
                0xF0..=0xF4 => {
                    // four bytes
                    match c {
                        // 0x80..=0x8F encodes overlong three byte codepoint
                        0xF0 => ensure_range!(1, 0x90..=0xBF),
                        // 0x90..=0xBF encodes codepoint > U+10FFFF
                        0xF4 => ensure_range!(1, 0x80..=0x8F),
                        // all UTF-8 continuation bytes are valid
                        _ => ensure_cont!(1),
                    }
                    ensure_cont!(2);
                    ensure_cont!(3);
                    Some(&buf[..4])
                }
                | 0x80..=0xBF // unicode continuation, invalid
                | 0xC0..=0xC1 // overlong one byte character
                | 0xF5..=0xF7 // four bytes that encode > U+10FFFF
                | 0xF8..=0xFB // five bytes, invalid
                | 0xFC..=0xFD // six bytes, invalid
                | 0xFE..=0xFF => Some(REPLACEMENT), // always invalid
            }
        }

        let mut buf = [0; 4];
        let mut result = Self::with_capacity(v.len());
        let mut iter = v.iter();
        while let Some(s) = next_char(&mut iter, &mut buf) {
            // SAFETY: next_char() only returns valid strings
            let s = unsafe { core::str::from_utf8_unchecked(s) };
            result.push_str(s);
        }
        result
    }

    fn from_utf16x(
        v: &[u8],
        from_int: impl Fn(u16) -> u16,
        from_bytes: impl Fn([u8; 2]) -> u16,
    ) -> Result<Self, Utf16Error> {
        if v.len() % 2 != 0 {
            // Input had an odd number of bytes.
            return Err(Utf16Error(()));
        }

        // Note: we don't use collect::<Result<_, _>>() because that fails to pre-allocate a buffer,
        // even though the size of our iterator, `v`, is known ahead of time.
        //
        // rustlang issue #48994 is tracking the fix
        let mut result = CompactString::with_capacity(v.len() / 2);

        // SAFETY: `u8` and `u16` are `Copy`, so if the alignment fits, we can transmute a
        //         `[u8; 2*N]` to `[u16; N]`. `slice::align_to()` checks if the alignment is right.
        match unsafe { v.align_to::<u16>() } {
            (&[], v, &[]) => {
                // Input is correctly aligned.
                for c in core::char::decode_utf16(v.iter().copied().map(from_int)) {
                    result.push(c.map_err(|_| Utf16Error(()))?);
                }
            }
            _ => {
                // Input's alignment is off.
                // SAFETY: we can always reinterpret a `[u8; 2*N]` slice as `[[u8; 2]; N]`
                let v = unsafe { slice::from_raw_parts(v.as_ptr().cast(), v.len() / 2) };
                for c in core::char::decode_utf16(v.iter().copied().map(from_bytes)) {
                    result.push(c.map_err(|_| Utf16Error(()))?);
                }
            }
        }

        Ok(result)
    }

    fn from_utf16x_lossy(
        v: &[u8],
        from_int: impl Fn(u16) -> u16,
        from_bytes: impl Fn([u8; 2]) -> u16,
    ) -> Self {
        // Notice: We write the string "�" instead of the character '�', so the character does not
        //         have to be formatted before it can be appended.

        let (trailing_extra_byte, v) = match v.len() % 2 != 0 {
            true => (true, &v[..v.len() - 1]),
            false => (false, v),
        };
        let mut result = CompactString::with_capacity(v.len() / 2);

        // SAFETY: `u8` and `u16` are `Copy`, so if the alignment fits, we can transmute a
        //         `[u8; 2*N]` to `[u16; N]`. `slice::align_to()` checks if the alignment is right.
        match unsafe { v.align_to::<u16>() } {
            (&[], v, &[]) => {
                // Input is correctly aligned.
                for c in core::char::decode_utf16(v.iter().copied().map(from_int)) {
                    match c {
                        Ok(c) => result.push(c),
                        Err(_) => result.push_str("�"),
                    }
                }
            }
            _ => {
                // Input's alignment is off.
                // SAFETY: we can always reinterpret a `[u8; 2*N]` slice as `[[u8; 2]; N]`
                let v = unsafe { slice::from_raw_parts(v.as_ptr().cast(), v.len() / 2) };
                for c in core::char::decode_utf16(v.iter().copied().map(from_bytes)) {
                    match c {
                        Ok(c) => result.push(c),
                        Err(_) => result.push_str("�"),
                    }
                }
            }
        }

        if trailing_extra_byte {
            result.push_str("�");
        }
        result
    }

    /// Decode a slice of bytes as UTF-16 encoded string, in little endian.
    ///
    /// # Errors
    ///
    /// If the slice has an odd number of bytes, or if it did not contain valid UTF-16 characters,
    /// a [`Utf16Error`] is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// const DANCING_MEN: &[u8] = b"\x3d\xd8\x6f\xdc\x0d\x20\x42\x26\x0f\xfe";
    /// let dancing_men = CompactString::from_utf16le(DANCING_MEN).unwrap();
    /// assert_eq!(dancing_men, "👯‍♂️");
    /// ```
    #[inline]
    pub fn from_utf16le(v: impl AsRef<[u8]>) -> Result<Self, Utf16Error> {
        CompactString::from_utf16x(v.as_ref(), u16::from_le, u16::from_le_bytes)
    }

    /// Decode a slice of bytes as UTF-16 encoded string, in big endian.
    ///
    /// # Errors
    ///
    /// If the slice has an odd number of bytes, or if it did not contain valid UTF-16 characters,
    /// a [`Utf16Error`] is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// const DANCING_WOMEN: &[u8] = b"\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xfe\x0f";
    /// let dancing_women = CompactString::from_utf16be(DANCING_WOMEN).unwrap();
    /// assert_eq!(dancing_women, "👯‍♀️");
    /// ```
    #[inline]
    pub fn from_utf16be(v: impl AsRef<[u8]>) -> Result<Self, Utf16Error> {
        CompactString::from_utf16x(v.as_ref(), u16::from_be, u16::from_be_bytes)
    }

    /// Lossy decode a slice of bytes as UTF-16 encoded string, in little endian.
    ///
    /// In this context "lossy" means that any broken characters in the input are replaced by the
    /// \<REPLACEMENT CHARACTER\> `'�'`. Please notice that, unlike UTF-8, UTF-16 is not self
    /// synchronizing. I.e. if a byte in the input is dropped, all following data is broken.
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// // A "random" bit was flipped in the 4th byte:
    /// const DANCING_MEN: &[u8] = b"\x3d\xd8\x6f\xfc\x0d\x20\x42\x26\x0f\xfe";
    /// let dancing_men = CompactString::from_utf16le_lossy(DANCING_MEN);
    /// assert_eq!(dancing_men, "�\u{fc6f}\u{200d}♂️");
    /// ```
    #[inline]
    pub fn from_utf16le_lossy(v: impl AsRef<[u8]>) -> Self {
        CompactString::from_utf16x_lossy(v.as_ref(), u16::from_le, u16::from_le_bytes)
    }

    /// Lossy decode a slice of bytes as UTF-16 encoded string, in big endian.
    ///
    /// In this context "lossy" means that any broken characters in the input are replaced by the
    /// \<REPLACEMENT CHARACTER\> `'�'`. Please notice that, unlike UTF-8, UTF-16 is not self
    /// synchronizing. I.e. if a byte in the input is dropped, all following data is broken.
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// // A "random" bit was flipped in the 9th byte:
    /// const DANCING_WOMEN: &[u8] = b"\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xde\x0f";
    /// let dancing_women = CompactString::from_utf16be_lossy(DANCING_WOMEN);
    /// assert_eq!(dancing_women, "👯\u{200d}♀�");
    /// ```
    #[inline]
    pub fn from_utf16be_lossy(v: impl AsRef<[u8]>) -> Self {
        CompactString::from_utf16x_lossy(v.as_ref(), u16::from_be, u16::from_be_bytes)
    }

    /// Convert the [`CompactString`] into a [`String`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use compact_str::CompactString;
    /// let s = CompactString::new("Hello world");
    /// let s = s.into_string();
    /// assert_eq!(s, "Hello world");
    /// ```
    pub fn into_string(self) -> String {
        self.0.into_string()
    }

    /// Convert a [`String`] into a [`CompactString`] _without inlining_.
    ///
    /// Note: You probably don't need to use this method, instead you should use `From<String>`
    /// which is implemented for [`CompactString`].
    ///
    /// This method exists incase your code is very sensitive to memory allocations. Normally when
    /// converting a [`String`] to a [`CompactString`] we'll inline short strings onto the stack.
    /// But this results in [`Drop`]-ing the original [`String`], which causes memory it owned on
    /// the heap to be deallocated. Instead when using this method, we always reuse the buffer that
    /// was previously owned by the [`String`], so no trips to the allocator are needed.
    ///
    /// # Examples
    ///
    /// ### Short Strings
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let short = "hello world".to_string();
    /// let c_heap = CompactString::from_string_buffer(short);
    ///
    /// // using CompactString::from_string_buffer, we'll re-use the String's underlying buffer
    /// assert!(c_heap.is_heap_allocated());
    ///
    /// // note: when Clone-ing a short heap allocated string, we'll eagerly inline at that point
    /// let c_inline = c_heap.clone();
    /// assert!(!c_inline.is_heap_allocated());
    ///
    /// assert_eq!(c_heap, c_inline);
    /// ```
    ///
    /// ### Longer Strings
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let x = "longer string that will be on the heap".to_string();
    /// let c1 = CompactString::from(x);
    ///
    /// let y = "longer string that will be on the heap".to_string();
    /// let c2 = CompactString::from_string_buffer(y);
    ///
    /// // for longer strings, we re-use the underlying String's buffer in both cases
    /// assert!(c1.is_heap_allocated());
    /// assert!(c2.is_heap_allocated());
    /// ```
    ///
    /// ### Buffer Re-use
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let og = "hello world".to_string();
    /// let og_addr = og.as_ptr();
    ///
    /// let mut c = CompactString::from_string_buffer(og);
    /// let ex_addr = c.as_ptr();
    ///
    /// // When converting to/from String and CompactString with from_string_buffer we always re-use
    /// // the same underlying allocated memory/buffer
    /// assert_eq!(og_addr, ex_addr);
    ///
    /// let long = "this is a long string that will be on the heap".to_string();
    /// let long_addr = long.as_ptr();
    ///
    /// let mut long_c = CompactString::from(long);
    /// let long_ex_addr = long_c.as_ptr();
    ///
    /// // When converting to/from String and CompactString with From<String>, we'll also re-use the
    /// // underlying buffer, if the string is long, otherwise when converting to CompactString we
    /// // eagerly inline
    /// assert_eq!(long_addr, long_ex_addr);
    /// ```
    #[inline]
    #[track_caller]
    pub fn from_string_buffer(s: String) -> Self {
        let repr = Repr::from_string(s, false).unwrap_with_msg();
        CompactString(repr)
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII lower case equivalent.
    ///
    /// ASCII letters 'A' to 'Z' are mapped to 'a' to 'z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To lowercase the value in-place, use [`str::make_ascii_lowercase`].
    ///
    /// To lowercase ASCII characters in addition to non-ASCII characters, use
    /// [`CompactString::to_lowercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let s = CompactString::new("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("grüße, jürgen ❤", s.to_ascii_lowercase());
    /// ```
    #[must_use = "to lowercase the value in-place, use `make_ascii_lowercase()`"]
    #[inline]
    pub fn to_ascii_lowercase(&self) -> Self {
        let mut s = self.clone();
        s.make_ascii_lowercase();
        s
    }

    /// Returns a copy of this string where each character is mapped to its
    /// ASCII upper case equivalent.
    ///
    /// ASCII letters 'a' to 'z' are mapped to 'A' to 'Z',
    /// but non-ASCII letters are unchanged.
    ///
    /// To uppercase the value in-place, use [`str::make_ascii_uppercase`].
    ///
    /// To uppercase ASCII characters in addition to non-ASCII characters, use
    /// [`CompactString::to_uppercase`].
    ///
    /// # Examples
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let s = CompactString::new("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s.to_ascii_uppercase());
    /// ```
    #[must_use = "to uppercase the value in-place, use `make_ascii_uppercase()`"]
    #[inline]
    pub fn to_ascii_uppercase(&self) -> Self {
        let mut s = self.clone();
        s.make_ascii_uppercase();
        s
    }

    /// Returns the lowercase equivalent of this string slice, as a new [`CompactString`].
    ///
    /// 'Lowercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Lowercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`CompactString`] instead of modifying the
    /// parameter in-place.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let s = CompactString::new("HELLO");
    ///
    /// assert_eq!("hello", s.to_lowercase());
    /// ```
    ///
    /// A tricky example, with sigma:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let sigma = CompactString::new("Σ");
    ///
    /// assert_eq!("σ", sigma.to_lowercase());
    ///
    /// // but at the end of a word, it's ς, not σ:
    /// let odysseus = CompactString::new("ὈΔΥΣΣΕΎΣ");
    ///
    /// assert_eq!("ὀδυσσεύς", odysseus.to_lowercase());
    /// ```
    ///
    /// Languages without case are not changed:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let new_year = CompactString::new("农历新年");
    ///
    /// assert_eq!(new_year, new_year.to_lowercase());
    /// ```
    #[must_use = "this returns the lowercase string as a new CompactString, \
                  without modifying the original"]
    pub fn to_lowercase(&self) -> Self {
        Self::from_str_to_lowercase(self.as_str())
    }

    /// Returns the lowercase equivalent of this string slice, as a new [`CompactString`].
    ///
    /// 'Lowercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Lowercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`CompactString`] instead of modifying the
    /// parameter in-place.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::CompactString;
    ///
    /// assert_eq!("hello", CompactString::from_str_to_lowercase("HELLO"));
    /// ```
    ///
    /// A tricky example, with sigma:
    ///
    /// ```
    /// use compact_str::CompactString;
    ///
    /// assert_eq!("σ", CompactString::from_str_to_lowercase("Σ"));
    ///
    /// // but at the end of a word, it's ς, not σ:
    /// assert_eq!("ὀδυσσεύς", CompactString::from_str_to_lowercase("ὈΔΥΣΣΕΎΣ"));
    /// ```
    ///
    /// Languages without case are not changed:
    ///
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let new_year = "农历新年";
    /// assert_eq!(new_year, CompactString::from_str_to_lowercase(new_year));
    /// ```
    #[must_use = "this returns the lowercase string as a new CompactString, \
                  without modifying the original"]
    pub fn from_str_to_lowercase(input: &str) -> Self {
        let mut s = convert_while_ascii(input.as_bytes(), u8::to_ascii_lowercase);

        // Safety: we know this is a valid char boundary since
        // out.len() is only progressed if ascii bytes are found
        let rest = unsafe { input.get_unchecked(s.len()..) };

        for (i, c) in rest.char_indices() {
            if c == 'Σ' {
                // Σ maps to σ, except at the end of a word where it maps to ς.
                // This is the only conditional (contextual) but language-independent mapping
                // in `SpecialCasing.txt`,
                // so hard-code it rather than have a generic "condition" mechanism.
                // See https://github.com/rust-lang/rust/issues/26035
                map_uppercase_sigma(rest, i, &mut s)
            } else {
                s.extend(c.to_lowercase());
            }
        }
        return s;

        fn map_uppercase_sigma(from: &str, i: usize, to: &mut CompactString) {
            // See https://www.unicode.org/versions/Unicode7.0.0/ch03.pdf#G33992
            // for the definition of `Final_Sigma`.
            debug_assert!('Σ'.len_utf8() == 2);
            let is_word_final = case_ignorable_then_cased(from[..i].chars().rev())
                && !case_ignorable_then_cased(from[i + 2..].chars());
            to.push_str(if is_word_final { "ς" } else { "σ" });
        }

        fn case_ignorable_then_cased<I: Iterator<Item = char>>(mut iter: I) -> bool {
            use unicode_data::case_ignorable::lookup as Case_Ignorable;
            use unicode_data::cased::lookup as Cased;
            match iter.find(|&c| !Case_Ignorable(c)) {
                Some(c) => Cased(c),
                None => false,
            }
        }
    }

    /// Returns the uppercase equivalent of this string slice, as a new [`CompactString`].
    ///
    /// 'Uppercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Uppercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`CompactString`] instead of modifying the
    /// parameter in-place.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let s = CompactString::new("hello");
    ///
    /// assert_eq!("HELLO", s.to_uppercase());
    /// ```
    ///
    /// Scripts without case are not changed:
    ///
    /// ```
    /// use compact_str::CompactString;
    /// let new_year = CompactString::new("农历新年");
    ///
    /// assert_eq!(new_year, new_year.to_uppercase());
    /// ```
    ///
    /// One character can become multiple:
    /// ```
    /// use compact_str::CompactString;
    /// let s = CompactString::new("tschüß");
    ///
    /// assert_eq!("TSCHÜSS", s.to_uppercase());
    /// ```
    #[must_use = "this returns the uppercase string as a new CompactString, \
                  without modifying the original"]
    pub fn to_uppercase(&self) -> Self {
        Self::from_str_to_uppercase(self.as_str())
    }

    /// Returns the uppercase equivalent of this string slice, as a new [`CompactString`].
    ///
    /// 'Uppercase' is defined according to the terms of the Unicode Derived Core Property
    /// `Uppercase`.
    ///
    /// Since some characters can expand into multiple characters when changing
    /// the case, this function returns a [`CompactString`] instead of modifying the
    /// parameter in-place.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use compact_str::CompactString;
    ///
    /// assert_eq!("HELLO", CompactString::from_str_to_uppercase("hello"));
    /// ```
    ///
    /// Scripts without case are not changed:
    ///
    /// ```
    /// use compact_str::CompactString;
    ///
    /// let new_year = "农历新年";
    /// assert_eq!(new_year, CompactString::from_str_to_uppercase(new_year));
    /// ```
    ///
    /// One character can become multiple:
    /// ```
    /// use compact_str::CompactString;
    ///
    /// assert_eq!("TSCHÜSS", CompactString::from_str_to_uppercase("tschüß"));
    /// ```
    #[must_use = "this returns the uppercase string as a new CompactString, \
                  without modifying the original"]
    pub fn from_str_to_uppercase(input: &str) -> Self {
        let mut out = convert_while_ascii(input.as_bytes(), u8::to_ascii_uppercase);

        // Safety: we know this is a valid char boundary since
        // out.len() is only progressed if ascii bytes are found
        let rest = unsafe { input.get_unchecked(out.len()..) };

        for c in rest.chars() {
            out.extend(c.to_uppercase());
        }

        out
    }
}

/// Converts the bytes while the bytes are still ascii.
/// For better average performance, this is happens in chunks of `2*size_of::<usize>()`.
/// Returns a vec with the converted bytes.
///
/// Copied from https://doc.rust-lang.org/nightly/src/alloc/str.rs.html#623-666
#[inline]
fn convert_while_ascii(b: &[u8], convert: fn(&u8) -> u8) -> CompactString {
    let mut out = CompactString::with_capacity(b.len());

    const USIZE_SIZE: usize = mem::size_of::<usize>();
    const MAGIC_UNROLL: usize = 2;
    const N: usize = USIZE_SIZE * MAGIC_UNROLL;
    const NONASCII_MASK: usize = usize::from_ne_bytes([0x80; USIZE_SIZE]);

    let mut i = 0;
    unsafe {
        while i + N <= b.len() {
            // Safety: we have checks the sizes `b` and `out` to know that our
            let in_chunk = b.get_unchecked(i..i + N);
            let out_chunk = out.spare_capacity_mut().get_unchecked_mut(i..i + N);

            let mut bits = 0;
            for j in 0..MAGIC_UNROLL {
                // read the bytes 1 usize at a time (unaligned since we haven't checked the
                // alignment) safety: in_chunk is valid bytes in the range
                bits |= in_chunk.as_ptr().cast::<usize>().add(j).read_unaligned();
            }
            // if our chunks aren't ascii, then return only the prior bytes as init
            if bits & NONASCII_MASK != 0 {
                break;
            }

            // perform the case conversions on N bytes (gets heavily autovec'd)
            for j in 0..N {
                // safety: in_chunk and out_chunk is valid bytes in the range
                let out = out_chunk.get_unchecked_mut(j);
                out.write(convert(in_chunk.get_unchecked(j)));
            }

            // mark these bytes as initialised
            i += N;
        }
        out.set_len(i);
    }

    out
}

impl Clone for CompactString {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0)
    }
}

impl Default for CompactString {
    #[inline]
    fn default() -> Self {
        CompactString::new("")
    }
}

impl Deref for CompactString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl DerefMut for CompactString {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl AsRef<str> for CompactString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "std")]
impl AsRef<OsStr> for CompactString {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        OsStr::new(self.as_str())
    }
}

impl AsRef<[u8]> for CompactString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Borrow<str> for CompactString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl BorrowMut<str> for CompactString {
    #[inline]
    fn borrow_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl Eq for CompactString {}

impl<T: AsRef<str> + ?Sized> PartialEq<T> for CompactString {
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}

impl PartialEq<CompactString> for &CompactString {
    fn eq(&self, other: &CompactString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<CompactString> for String {
    fn eq(&self, other: &CompactString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'a> PartialEq<&'a CompactString> for String {
    fn eq(&self, other: &&CompactString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<CompactString> for &String {
    fn eq(&self, other: &CompactString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<CompactString> for str {
    fn eq(&self, other: &CompactString) -> bool {
        self == other.as_str()
    }
}

impl<'a> PartialEq<&'a CompactString> for str {
    fn eq(&self, other: &&CompactString) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<CompactString> for &str {
    fn eq(&self, other: &CompactString) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<CompactString> for &&str {
    fn eq(&self, other: &CompactString) -> bool {
        **self == other.as_str()
    }
}

impl<'a> PartialEq<CompactString> for Cow<'a, str> {
    fn eq(&self, other: &CompactString) -> bool {
        *self == other.as_str()
    }
}

impl<'a> PartialEq<CompactString> for &Cow<'a, str> {
    fn eq(&self, other: &CompactString) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for &CompactString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'a> PartialEq<Cow<'a, str>> for &CompactString {
    fn eq(&self, other: &Cow<'a, str>) -> bool {
        self.as_str() == other
    }
}

impl Ord for CompactString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for CompactString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for CompactString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl<'a> From<&'a str> for CompactString {
    #[inline]
    #[track_caller]
    fn from(s: &'a str) -> Self {
        CompactString::new(s)
    }
}

impl From<String> for CompactString {
    #[inline]
    #[track_caller]
    fn from(s: String) -> Self {
        let repr = Repr::from_string(s, true).unwrap_with_msg();
        CompactString(repr)
    }
}

impl<'a> From<&'a String> for CompactString {
    #[inline]
    #[track_caller]
    fn from(s: &'a String) -> Self {
        CompactString::new(s)
    }
}

impl<'a> From<Cow<'a, str>> for CompactString {
    fn from(cow: Cow<'a, str>) -> Self {
        match cow {
            Cow::Borrowed(s) => s.into(),
            // we separate these two so we can re-use the underlying buffer in the owned case
            Cow::Owned(s) => s.into(),
        }
    }
}

impl From<Box<str>> for CompactString {
    #[inline]
    #[track_caller]
    fn from(b: Box<str>) -> Self {
        let s = b.into_string();
        let repr = Repr::from_string(s, true).unwrap_with_msg();
        CompactString(repr)
    }
}

impl From<CompactString> for String {
    #[inline]
    fn from(s: CompactString) -> Self {
        s.into_string()
    }
}

impl From<CompactString> for Cow<'_, str> {
    #[inline]
    fn from(s: CompactString) -> Self {
        if let Some(s) = s.as_static_str() {
            Self::Borrowed(s)
        } else {
            Self::Owned(s.into_string())
        }
    }
}

impl<'a> From<&'a CompactString> for Cow<'a, str> {
    #[inline]
    fn from(s: &'a CompactString) -> Self {
        Self::Borrowed(s)
    }
}

#[cfg(target_has_atomic = "ptr")]
impl From<CompactString> for alloc::sync::Arc<str> {
    fn from(value: CompactString) -> Self {
        Self::from(value.as_str())
    }
}

impl From<CompactString> for alloc::rc::Rc<str> {
    fn from(value: CompactString) -> Self {
        Self::from(value.as_str())
    }
}

#[cfg(feature = "std")]
impl From<CompactString> for Box<dyn std::error::Error + Send + Sync> {
    fn from(value: CompactString) -> Self {
        struct StringError(CompactString);

        impl std::error::Error for StringError {
            #[allow(deprecated)]
            fn description(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for StringError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        // Purposefully skip printing "StringError(..)"
        impl fmt::Debug for StringError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        Box::new(StringError(value))
    }
}

#[cfg(feature = "std")]
impl From<CompactString> for Box<dyn std::error::Error> {
    fn from(value: CompactString) -> Self {
        let err1: Box<dyn std::error::Error + Send + Sync> = From::from(value);
        let err2: Box<dyn std::error::Error> = err1;
        err2
    }
}

impl From<CompactString> for Box<str> {
    fn from(value: CompactString) -> Self {
        if value.is_heap_allocated() {
            value.into_string().into_boxed_str()
        } else {
            Box::from(value.as_str())
        }
    }
}

#[cfg(feature = "std")]
impl From<CompactString> for std::ffi::OsString {
    fn from(value: CompactString) -> Self {
        Self::from(value.into_string())
    }
}

#[cfg(feature = "std")]
impl From<CompactString> for std::path::PathBuf {
    fn from(value: CompactString) -> Self {
        Self::from(std::ffi::OsString::from(value))
    }
}

#[cfg(feature = "std")]
impl AsRef<std::path::Path> for CompactString {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}

impl From<CompactString> for alloc::vec::Vec<u8> {
    fn from(value: CompactString) -> Self {
        if value.is_heap_allocated() {
            value.into_string().into_bytes()
        } else {
            value.as_bytes().to_vec()
        }
    }
}

impl FromStr for CompactString {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<CompactString, Self::Err> {
        Ok(CompactString::from(s))
    }
}

impl fmt::Debug for CompactString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for CompactString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl FromIterator<char> for CompactString {
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl<'a> FromIterator<&'a char> for CompactString {
    fn from_iter<T: IntoIterator<Item = &'a char>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl<'a> FromIterator<&'a str> for CompactString {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl FromIterator<Box<str>> for CompactString {
    fn from_iter<T: IntoIterator<Item = Box<str>>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl<'a> FromIterator<Cow<'a, str>> for CompactString {
    fn from_iter<T: IntoIterator<Item = Cow<'a, str>>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl FromIterator<String> for CompactString {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl FromIterator<CompactString> for CompactString {
    fn from_iter<T: IntoIterator<Item = CompactString>>(iter: T) -> Self {
        let repr = iter.into_iter().collect();
        CompactString(repr)
    }
}

impl FromIterator<CompactString> for String {
    fn from_iter<T: IntoIterator<Item = CompactString>>(iter: T) -> Self {
        let mut iterator = iter.into_iter();
        match iterator.next() {
            None => String::new(),
            Some(buf) => {
                let mut buf = buf.into_string();
                buf.extend(iterator);
                buf
            }
        }
    }
}

impl FromIterator<CompactString> for Cow<'_, str> {
    fn from_iter<T: IntoIterator<Item = CompactString>>(iter: T) -> Self {
        String::from_iter(iter).into()
    }
}

impl Extend<char> for CompactString {
    fn extend<T: IntoIterator<Item = char>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl<'a> Extend<&'a char> for CompactString {
    fn extend<T: IntoIterator<Item = &'a char>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl<'a> Extend<&'a str> for CompactString {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl Extend<Box<str>> for CompactString {
    fn extend<T: IntoIterator<Item = Box<str>>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl<'a> Extend<Cow<'a, str>> for CompactString {
    fn extend<T: IntoIterator<Item = Cow<'a, str>>>(&mut self, iter: T) {
        iter.into_iter().for_each(move |s| self.push_str(&s));
    }
}

impl Extend<String> for CompactString {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl Extend<CompactString> for String {
    fn extend<T: IntoIterator<Item = CompactString>>(&mut self, iter: T) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl Extend<CompactString> for CompactString {
    fn extend<T: IntoIterator<Item = CompactString>>(&mut self, iter: T) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl<'a> Extend<CompactString> for Cow<'a, str> {
    fn extend<T: IntoIterator<Item = CompactString>>(&mut self, iter: T) {
        self.to_mut().extend(iter);
    }
}

impl fmt::Write for CompactString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    fn write_fmt(mut self: &mut Self, args: fmt::Arguments<'_>) -> fmt::Result {
        match args.as_str() {
            Some(s) => {
                if self.is_empty() && !self.is_heap_allocated() {
                    // Since self is currently an empty inline variant or
                    // an empty `StaticStr` variant, constructing a new one
                    // with `Self::const_new` is more efficient since
                    // it is guaranteed to be O(1).
                    *self = Self::const_new(s);
                } else {
                    self.push_str(s);
                }
                Ok(())
            }
            None => fmt::write(&mut self, args),
        }
    }
}

impl Add<&str> for CompactString {
    type Output = Self;
    fn add(mut self, rhs: &str) -> Self::Output {
        self.push_str(rhs);
        self
    }
}

impl AddAssign<&str> for CompactString {
    fn add_assign(&mut self, rhs: &str) {
        self.push_str(rhs);
    }
}

/// A possible error value when converting a [`CompactString`] from a UTF-16 byte slice.
///
/// This type is the error type for the [`from_utf16`] method on [`CompactString`].
///
/// [`from_utf16`]: CompactString::from_utf16
/// # Examples
///
/// Basic usage:
///
/// ```
/// # use compact_str::CompactString;
/// // 𝄞mu<invalid>ic
/// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075,
///           0xD800, 0x0069, 0x0063];
///
/// assert!(CompactString::from_utf16(v).is_err());
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Utf16Error(());

impl fmt::Display for Utf16Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt("invalid utf-16: lone surrogate found", f)
    }
}

/// An iterator over the exacted data by [`CompactString::drain()`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Drain<'a> {
    compact_string: *mut CompactString,
    start: usize,
    end: usize,
    chars: core::str::Chars<'a>,
}

// SAFETY: Drain keeps the lifetime of the CompactString it belongs to.
unsafe impl Send for Drain<'_> {}
unsafe impl Sync for Drain<'_> {}

impl fmt::Debug for Drain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Drain").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Drain<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Drop for Drain<'_> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: Drain keeps a mutable reference to compact_string, so one one else can access
        //         the CompactString, but this function right now. CompactString::drain() ensured
        //         that the new extracted range does not split a UTF-8 character.
        unsafe { (*self.compact_string).replace_range_shrink(self.start, self.end, "") };
    }
}

impl Drain<'_> {
    /// The remaining, unconsumed characters of the extracted substring.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.chars.as_str()
    }
}

impl Deref for Drain<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Iterator for Drain<'_> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }

    #[inline]
    fn count(self) -> usize {
        // <Chars as Iterator>::count() is specialized, and cloning is trivial.
        self.chars.clone().count()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chars.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<char> {
        self.chars.next_back()
    }
}

impl DoubleEndedIterator for Drain<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        self.chars.next_back()
    }
}

impl FusedIterator for Drain<'_> {}

/// A possible error value if allocating or resizing a [`CompactString`] failed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReserveError(());

impl fmt::Display for ReserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cannot allocate memory to hold CompactString")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for ReserveError {}

/// A possible error value if [`ToCompactString::try_to_compact_string()`] failed.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ToCompactStringError {
    /// Cannot allocate memory to hold CompactString
    Reserve(ReserveError),
    /// [`Display::fmt()`][core::fmt::Display::fmt] returned an error
    Fmt(fmt::Error),
}

impl fmt::Display for ToCompactStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToCompactStringError::Reserve(err) => err.fmt(f),
            ToCompactStringError::Fmt(err) => err.fmt(f),
        }
    }
}

impl From<ReserveError> for ToCompactStringError {
    #[inline]
    fn from(value: ReserveError) -> Self {
        Self::Reserve(value)
    }
}

impl From<fmt::Error> for ToCompactStringError {
    #[inline]
    fn from(value: fmt::Error) -> Self {
        Self::Fmt(value)
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl std::error::Error for ToCompactStringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToCompactStringError::Reserve(err) => Some(err),
            ToCompactStringError::Fmt(err) => Some(err),
        }
    }
}

trait UnwrapWithMsg {
    type T;

    fn unwrap_with_msg(self) -> Self::T;
}

impl<T, E: fmt::Display> UnwrapWithMsg for Result<T, E> {
    type T = T;

    #[inline(always)]
    #[track_caller]
    fn unwrap_with_msg(self) -> T {
        match self {
            Ok(value) => value,
            Err(err) => unwrap_with_msg_fail(err),
        }
    }
}

#[inline(never)]
#[cold]
#[track_caller]
fn unwrap_with_msg_fail<E: fmt::Display>(error: E) -> ! {
    panic!("{error}")
}

static_assertions::assert_eq_size!(CompactString, String);
