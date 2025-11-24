// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A UTF-8 encoded, growable string.
//!
//! This module contains the [`String`] type and several error types that may
//! result from working with [`String`]s.
//!
//! This module is a fork of the [`std::string`] module, that uses a bump allocator.
//!
//! [`std::string`]: https://doc.rust-lang.org/std/string/index.html
//!
//! # Examples
//!
//! You can create a new [`String`] from a string literal with [`String::from_str_in`]:
//!
//! ```
//! use bumpalo::{Bump, collections::String};
//!
//! let b = Bump::new();
//!
//! let s = String::from_str_in("world", &b);
//! ```
//!
//! [`String`]: struct.String.html
//! [`String::from_str_in`]: struct.String.html#method.from_str_in
//!
//! If you have a vector of valid UTF-8 bytes, you can make a [`String`] out of
//! it. You can do the reverse too.
//!
//! ```
//! use bumpalo::{Bump, collections::String};
//!
//! let b = Bump::new();
//!
//! let sparkle_heart = bumpalo::vec![in &b; 240, 159, 146, 150];
//!
//! // We know these bytes are valid, so we'll use `unwrap()`.
//! let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
//!
//! assert_eq!("💖", sparkle_heart);
//!
//! let bytes = sparkle_heart.into_bytes();
//!
//! assert_eq!(bytes, [240, 159, 146, 150]);
//! ```

use crate::collections::str::lossy;
use crate::collections::vec::Vec;
use crate::Bump;
use core::borrow::{Borrow, BorrowMut};
use core::char::decode_utf16;
use core::fmt;
use core::hash;
use core::iter::FusedIterator;
use core::mem;
use core::ops::Bound::{Excluded, Included, Unbounded};
use core::ops::{self, Add, AddAssign, Index, IndexMut, RangeBounds};
use core::ptr;
use core::str::{self, Chars, Utf8Error};
use core_alloc::borrow::Cow;

/// Like the [`format!`] macro, but for creating [`bumpalo::collections::String`]s.
///
/// [`format!`]: https://doc.rust-lang.org/std/macro.format.html
/// [`bumpalo::collections::String`]: collections/string/struct.String.html
///
/// # Examples
///
/// ```
/// use bumpalo::Bump;
///
/// let b = Bump::new();
///
/// let who = "World";
/// let s = bumpalo::format!(in &b, "Hello, {}!", who);
/// assert_eq!(s, "Hello, World!")
/// ```
#[macro_export]
macro_rules! format {
    ( in $bump:expr, $fmt:expr, $($args:expr),* ) => {{
        use $crate::core_alloc::fmt::Write;
        let bump = $bump;
        let mut s = $crate::collections::String::new_in(bump);
        let _ = write!(&mut s, $fmt, $($args),*);
        s
    }};

    ( in $bump:expr, $fmt:expr, $($args:expr,)* ) => {
        $crate::format!(in $bump, $fmt, $($args),*)
    };
}

/// A UTF-8 encoded, growable string.
///
/// The `String` type is the most common string type that has ownership over the
/// contents of the string. It has a close relationship with its borrowed
/// counterpart, the primitive [`str`].
///
/// [`str`]: https://doc.rust-lang.org/std/primitive.str.html
///
/// # Examples
///
/// You can create a `String` from a literal string with [`String::from_str_in`]:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// let hello = String::from_str_in("Hello, world!", &b);
/// ```
///
/// You can append a [`char`] to a `String` with the [`push`] method, and
/// append a [`&str`] with the [`push_str`] method:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// let mut hello = String::from_str_in("Hello, ", &b);
///
/// hello.push('w');
/// hello.push_str("orld!");
/// ```
///
/// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
/// [`push`]: #method.push
/// [`push_str`]: #method.push_str
///
/// If you have a vector of UTF-8 bytes, you can create a `String` from it with
/// the [`from_utf8`] method:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// // some bytes, in a vector
/// let sparkle_heart = bumpalo::vec![in &b; 240, 159, 146, 150];
///
/// // We know these bytes are valid, so we'll use `unwrap()`.
/// let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
///
/// assert_eq!("💖", sparkle_heart);
/// ```
///
/// [`from_utf8`]: #method.from_utf8
///
/// # Deref
///
/// `String`s implement <code>[`Deref`]<Target = [`str`]></code>, and so inherit all of [`str`]'s
/// methods. In addition, this means that you can pass a `String` to a
/// function which takes a [`&str`] by using an ampersand (`&`):
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// fn takes_str(s: &str) { }
///
/// let s = String::from_str_in("Hello", &b);
///
/// takes_str(&s);
/// ```
///
/// This will create a [`&str`] from the `String` and pass it in. This
/// conversion is very inexpensive, and so generally, functions will accept
/// [`&str`]s as arguments unless they need a `String` for some specific
/// reason.
///
/// In certain cases Rust doesn't have enough information to make this
/// conversion, known as [`Deref`] coercion. In the following example a string
/// slice [`&'a str`][`&str`] implements the trait `TraitExample`, and the function
/// `example_func` takes anything that implements the trait. In this case Rust
/// would need to make two implicit conversions, which Rust doesn't have the
/// means to do. For that reason, the following example will not compile.
///
/// ```compile_fail,E0277
/// use bumpalo::{Bump, collections::String};
///
/// trait TraitExample {}
///
/// impl<'a> TraitExample for &'a str {}
///
/// fn example_func<A: TraitExample>(example_arg: A) {}
///
/// let b = Bump::new();
/// let example_string = String::from_str_in("example_string", &b);
/// example_func(&example_string);
/// ```
///
/// There are two options that would work instead. The first would be to
/// change the line `example_func(&example_string);` to
/// `example_func(example_string.as_str());`, using the method [`as_str()`]
/// to explicitly extract the string slice containing the string. The second
/// way changes `example_func(&example_string);` to
/// `example_func(&*example_string);`. In this case we are dereferencing a
/// `String` to a [`str`][`&str`], then referencing the [`str`][`&str`] back to
/// [`&str`]. The second way is more idiomatic, however both work to do the
/// conversion explicitly rather than relying on the implicit conversion.
///
/// # Representation
///
/// A `String` is made up of three components: a pointer to some bytes, a
/// length, and a capacity. The pointer points to an internal buffer `String`
/// uses to store its data. The length is the number of bytes currently stored
/// in the buffer, and the capacity is the size of the buffer in bytes. As such,
/// the length will always be less than or equal to the capacity.
///
/// This buffer is always stored on the heap.
///
/// You can look at these with the [`as_ptr`], [`len`], and [`capacity`]
/// methods:
///
/// ```
/// use bumpalo::{Bump, collections::String};
/// use std::mem;
///
/// let b = Bump::new();
///
/// let mut story = String::from_str_in("Once upon a time...", &b);
///
/// let ptr = story.as_mut_ptr();
/// let len = story.len();
/// let capacity = story.capacity();
///
/// // story has nineteen bytes
/// assert_eq!(19, len);
///
/// // Now that we have our parts, we throw the story away.
/// mem::forget(story);
///
/// // We can re-build a String out of ptr, len, and capacity. This is all
/// // unsafe because we are responsible for making sure the components are
/// // valid:
/// let s = unsafe { String::from_raw_parts_in(ptr, len, capacity, &b) } ;
///
/// assert_eq!(String::from_str_in("Once upon a time...", &b), s);
/// ```
///
/// [`as_ptr`]: https://doc.rust-lang.org/std/primitive.str.html#method.as_ptr
/// [`len`]: #method.len
/// [`capacity`]: #method.capacity
///
/// If a `String` has enough capacity, adding elements to it will not
/// re-allocate. For example, consider this program:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// let mut s = String::new_in(&b);
///
/// println!("{}", s.capacity());
///
/// for _ in 0..5 {
///     s.push_str("hello");
///     println!("{}", s.capacity());
/// }
/// ```
///
/// This will output the following:
///
/// ```text
/// 0
/// 5
/// 10
/// 20
/// 20
/// 40
/// ```
///
/// At first, we have no memory allocated at all, but as we append to the
/// string, it increases its capacity appropriately. If we instead use the
/// [`with_capacity_in`] method to allocate the correct capacity initially:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// let mut s = String::with_capacity_in(25, &b);
///
/// println!("{}", s.capacity());
///
/// for _ in 0..5 {
///     s.push_str("hello");
///     println!("{}", s.capacity());
/// }
/// ```
///
/// [`with_capacity_in`]: #method.with_capacity_in
///
/// We end up with a different output:
///
/// ```text
/// 25
/// 25
/// 25
/// 25
/// 25
/// 25
/// ```
///
/// Here, there's no need to allocate more memory inside the loop.
///
/// [`&str`]: https://doc.rust-lang.org/std/primitive.str.html
/// [`Deref`]: https://doc.rust-lang.org/std/ops/trait.Deref.html
/// [`as_str()`]: struct.String.html#method.as_str
#[derive(PartialOrd, Eq, Ord)]
pub struct String<'bump> {
    vec: Vec<'bump, u8>,
}

/// A possible error value when converting a `String` from a UTF-8 byte vector.
///
/// This type is the error type for the [`from_utf8`] method on [`String`]. It
/// is designed in such a way to carefully avoid reallocations: the
/// [`into_bytes`] method will give back the byte vector that was used in the
/// conversion attempt.
///
/// [`from_utf8`]: struct.String.html#method.from_utf8
/// [`String`]: struct.String.html
/// [`into_bytes`]: struct.FromUtf8Error.html#method.into_bytes
///
/// The [`Utf8Error`] type provided by [`std::str`] represents an error that may
/// occur when converting a slice of [`u8`]s to a [`&str`]. In this sense, it's
/// an analogue to `FromUtf8Error`, and you can get one from a `FromUtf8Error`
/// through the [`utf8_error`] method.
///
/// [`Utf8Error`]: https://doc.rust-lang.org/std/str/struct.Utf8Error.html
/// [`std::str`]: https://doc.rust-lang.org/std/str/index.html
/// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
/// [`&str`]: https://doc.rust-lang.org/std/primitive.str.html
/// [`utf8_error`]: #method.utf8_error
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// // some invalid bytes, in a vector
/// let bytes = bumpalo::vec![in &b; 0, 159];
///
/// let value = String::from_utf8(bytes);
///
/// assert!(value.is_err());
/// assert_eq!(bumpalo::vec![in &b; 0, 159], value.unwrap_err().into_bytes());
/// ```
#[derive(Debug)]
pub struct FromUtf8Error<'bump> {
    bytes: Vec<'bump, u8>,
    error: Utf8Error,
}

/// A possible error value when converting a `String` from a UTF-16 byte slice.
///
/// This type is the error type for the [`from_utf16_in`] method on [`String`].
///
/// [`from_utf16_in`]: struct.String.html#method.from_utf16_in
/// [`String`]: struct.String.html
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let b = Bump::new();
///
/// // 𝄞mu<invalid>ic
/// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075, 0xD800, 0x0069, 0x0063];
///
/// assert!(String::from_utf16_in(v, &b).is_err());
/// ```
#[derive(Debug)]
pub struct FromUtf16Error(());

impl<'bump> String<'bump> {
    /// Creates a new empty `String`.
    ///
    /// Given that the `String` is empty, this will not allocate any initial
    /// buffer. While that means that this initial operation is very
    /// inexpensive, it may cause excessive allocation later when you add
    /// data. If you have an idea of how much data the `String` will hold,
    /// consider the [`with_capacity_in`] method to prevent excessive
    /// re-allocation.
    ///
    /// [`with_capacity_in`]: #method.with_capacity_in
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::new_in(&b);
    /// ```
    #[inline]
    pub fn new_in(bump: &'bump Bump) -> String<'bump> {
        String {
            vec: Vec::new_in(bump),
        }
    }

    /// Creates a new empty `String` with a particular capacity.
    ///
    /// `String`s have an internal buffer to hold their data. The capacity is
    /// the length of that buffer, and can be queried with the [`capacity`]
    /// method. This method creates an empty `String`, but one with an initial
    /// buffer that can hold `capacity` bytes. This is useful when you may be
    /// appending a bunch of data to the `String`, reducing the number of
    /// reallocations it needs to do.
    ///
    /// [`capacity`]: #method.capacity
    ///
    /// If the given capacity is `0`, no allocation will occur, and this method
    /// is identical to the [`new_in`] method.
    ///
    /// [`new_in`]: #method.new
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::with_capacity_in(10, &b);
    ///
    /// // The String contains no chars, even though it has capacity for more
    /// assert_eq!(s.len(), 0);
    ///
    /// // These are all done without reallocating...
    /// let cap = s.capacity();
    /// for _ in 0..10 {
    ///     s.push('a');
    /// }
    ///
    /// assert_eq!(s.capacity(), cap);
    ///
    /// // ...but this may make the vector reallocate
    /// s.push('a');
    /// ```
    #[inline]
    pub fn with_capacity_in(capacity: usize, bump: &'bump Bump) -> String<'bump> {
        String {
            vec: Vec::with_capacity_in(capacity, bump),
        }
    }

    /// Converts a vector of bytes to a `String`.
    ///
    /// A string (`String`) is made of bytes ([`u8`]), and a vector of bytes
    /// ([`Vec<u8>`]) is made of bytes, so this function converts between the
    /// two. Not all byte slices are valid `String`s, however: `String`
    /// requires that it is valid UTF-8. `from_utf8()` checks to ensure that
    /// the bytes are valid UTF-8, and then does the conversion.
    ///
    /// If you are sure that the byte slice is valid UTF-8, and you don't want
    /// to incur the overhead of the validity check, there is an unsafe version
    /// of this function, [`from_utf8_unchecked`], which has the same behavior
    /// but skips the check.
    ///
    /// This method will take care to not copy the vector, for efficiency's
    /// sake.
    ///
    /// If you need a [`&str`] instead of a `String`, consider
    /// [`str::from_utf8`].
    ///
    /// The inverse of this method is [`into_bytes`].
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the slice is not UTF-8 with a description as to why the
    /// provided bytes are not UTF-8. The vector you moved in is also included.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some bytes, in a vector
    /// let sparkle_heart = bumpalo::vec![in &b; 240, 159, 146, 150];
    ///
    /// // We know these bytes are valid, so we'll use `unwrap()`.
    /// let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some invalid bytes, in a vector
    /// let sparkle_heart = bumpalo::vec![in &b; 0, 159, 146, 150];
    ///
    /// assert!(String::from_utf8(sparkle_heart).is_err());
    /// ```
    ///
    /// See the docs for [`FromUtf8Error`] for more details on what you can do
    /// with this error.
    ///
    /// [`from_utf8_unchecked`]: struct.String.html#method.from_utf8_unchecked
    /// [`&str`]: https://doc.rust-lang.org/std/primitive.str.html
    /// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
    /// [`Vec<u8>`]: ../vec/struct.Vec.html
    /// [`str::from_utf8`]: https://doc.rust-lang.org/std/str/fn.from_utf8.html
    /// [`into_bytes`]: struct.String.html#method.into_bytes
    /// [`FromUtf8Error`]: struct.FromUtf8Error.html
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    #[inline]
    pub fn from_utf8(vec: Vec<'bump, u8>) -> Result<String<'bump>, FromUtf8Error<'bump>> {
        match str::from_utf8(&vec) {
            Ok(..) => Ok(String { vec }),
            Err(e) => Err(FromUtf8Error {
                bytes: vec,
                error: e,
            }),
        }
    }

    /// Converts a slice of bytes to a string, including invalid characters.
    ///
    /// Strings are made of bytes ([`u8`]), and a slice of bytes
    /// ([`&[u8]`][slice]) is made of bytes, so this function converts
    /// between the two. Not all byte slices are valid strings, however: strings
    /// are required to be valid UTF-8. During this conversion,
    /// `from_utf8_lossy_in()` will replace any invalid UTF-8 sequences with
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD], which looks like this: �
    ///
    /// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
    /// [slice]: https://doc.rust-lang.org/std/primitive.slice.html
    /// [U+FFFD]: https://doc.rust-lang.org/std/char/constant.REPLACEMENT_CHARACTER.html
    ///
    /// If you are sure that the byte slice is valid UTF-8, and you don't want
    /// to incur the overhead of the conversion, there is an unsafe version
    /// of this function, [`from_utf8_unchecked`], which has the same behavior
    /// but skips the checks.
    ///
    /// [`from_utf8_unchecked`]: struct.String.html#method.from_utf8_unchecked
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{collections::String, Bump, vec};
    ///
    /// let b = Bump::new();
    ///
    /// // some bytes, in a vector
    /// let sparkle_heart = bumpalo::vec![in &b; 240, 159, 146, 150];
    ///
    /// let sparkle_heart = String::from_utf8_lossy_in(&sparkle_heart, &b);
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    ///
    /// Incorrect bytes:
    ///
    /// ```
    /// use bumpalo::{collections::String, Bump, vec};
    ///
    /// let b = Bump::new();
    ///
    /// // some invalid bytes
    /// let input = b"Hello \xF0\x90\x80World";
    /// let output = String::from_utf8_lossy_in(input, &b);
    ///
    /// assert_eq!("Hello �World", output);
    /// ```
    pub fn from_utf8_lossy_in(v: &[u8], bump: &'bump Bump) -> String<'bump> {
        let mut iter = lossy::Utf8Lossy::from_bytes(v).chunks();

        let (first_valid, first_broken) = if let Some(chunk) = iter.next() {
            let lossy::Utf8LossyChunk { valid, broken } = chunk;
            if valid.len() == v.len() {
                debug_assert!(broken.is_empty());
                unsafe {
                    return String::from_utf8_unchecked(Vec::from_iter_in(v.iter().cloned(), bump));
                }
            }
            (valid, broken)
        } else {
            return String::from_str_in("", bump);
        };

        const REPLACEMENT: &str = "\u{FFFD}";

        let mut res = String::with_capacity_in(v.len(), bump);
        res.push_str(first_valid);
        if !first_broken.is_empty() {
            res.push_str(REPLACEMENT);
        }

        for lossy::Utf8LossyChunk { valid, broken } in iter {
            res.push_str(valid);
            if !broken.is_empty() {
                res.push_str(REPLACEMENT);
            }
        }

        res
    }

    /// Decode a UTF-16 encoded slice `v` into a `String`, returning [`Err`]
    /// if `v` contains any invalid data.
    ///
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // 𝄞music
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075, 0x0073, 0x0069, 0x0063];
    /// assert_eq!(String::from_str_in("𝄞music", &b), String::from_utf16_in(v, &b).unwrap());
    ///
    /// // 𝄞mu<invalid>ic
    /// let v = &[0xD834, 0xDD1E, 0x006d, 0x0075, 0xD800, 0x0069, 0x0063];
    /// assert!(String::from_utf16_in(v, &b).is_err());
    /// ```
    pub fn from_utf16_in(v: &[u16], bump: &'bump Bump) -> Result<String<'bump>, FromUtf16Error> {
        let mut ret = String::with_capacity_in(v.len(), bump);
        for c in decode_utf16(v.iter().cloned()) {
            if let Ok(c) = c {
                ret.push(c);
            } else {
                return Err(FromUtf16Error(()));
            }
        }
        Ok(ret)
    }

    /// Construct a new `String<'bump>` from a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_str_in("hello", &b);
    /// assert_eq!(s, "hello");
    /// ```
    pub fn from_str_in(s: &str, bump: &'bump Bump) -> String<'bump> {
        let len = s.len();
        let mut t = String::with_capacity_in(len, bump);
        // SAFETY:
        // * `src` is valid for reads of `s.len()` bytes by virtue of being an allocated `&str`.
        // * `dst` is valid for writes of `s.len()` bytes as `String::with_capacity_in(s.len(), bump)`
        //   above guarantees that.
        // * Alignment is not relevant as `u8` has no alignment requirements.
        // * Source and destination ranges cannot overlap as we just reserved the destination
        //   range from the bump.
        unsafe { ptr::copy_nonoverlapping(s.as_ptr(), t.vec.as_mut_ptr(), len) };
        // SAFETY: We reserved sufficent capacity for the string above.
        // The elements at `0..len` were initialized by `copy_nonoverlapping` above.
        unsafe { t.vec.set_len(len) };
        t
    }

    /// Construct a new `String<'bump>` from an iterator of `char`s.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_iter_in(['h', 'e', 'l', 'l', 'o'].iter().cloned(), &b);
    /// assert_eq!(s, "hello");
    /// ```
    pub fn from_iter_in<I: IntoIterator<Item = char>>(iter: I, bump: &'bump Bump) -> String<'bump> {
        let mut s = String::new_in(bump);
        for c in iter {
            s.push(c);
        }
        s
    }

    /// Creates a new `String` from a length, capacity, and pointer.
    ///
    /// # Safety
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * The memory at `ptr` needs to have been previously allocated by the
    ///   same allocator the standard library uses.
    /// * `length` needs to be less than or equal to `capacity`.
    /// * `capacity` needs to be the correct value.
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `String` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    /// use std::mem;
    ///
    /// let b = Bump::new();
    ///
    /// unsafe {
    ///     let mut s = String::from_str_in("hello", &b);
    ///     let ptr = s.as_mut_ptr();
    ///     let len = s.len();
    ///     let capacity = s.capacity();
    ///
    ///     mem::forget(s);
    ///
    ///     let s = String::from_raw_parts_in(ptr, len, capacity, &b);
    ///
    ///     assert_eq!(s, "hello");
    /// }
    /// ```
    #[inline]
    pub unsafe fn from_raw_parts_in(
        buf: *mut u8,
        length: usize,
        capacity: usize,
        bump: &'bump Bump,
    ) -> String<'bump> {
        String {
            vec: Vec::from_raw_parts_in(buf, length, capacity, bump),
        }
    }

    /// Converts a vector of bytes to a `String` without checking that the
    /// string contains valid UTF-8.
    ///
    /// See the safe version, [`from_utf8`], for more details.
    ///
    /// [`from_utf8`]: struct.String.html#method.from_utf8
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not check that the bytes passed
    /// to it are valid UTF-8. If this constraint is violated, it may cause
    /// memory unsafety issues with future users of the `String`,
    /// as it is assumed that `String`s are valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some bytes, in a vector
    /// let sparkle_heart = bumpalo::vec![in &b; 240, 159, 146, 150];
    ///
    /// let sparkle_heart = unsafe {
    ///     String::from_utf8_unchecked(sparkle_heart)
    /// };
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    #[inline]
    pub unsafe fn from_utf8_unchecked(bytes: Vec<'bump, u8>) -> String<'bump> {
        String { vec: bytes }
    }

    /// Returns a shared reference to the allocator backing this `String`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// // uses the same allocator as the provided `String`
    /// fn copy_string<'bump>(s: &String<'bump>) -> &'bump str {
    ///     s.bump().alloc_str(s.as_str())
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn bump(&self) -> &'bump Bump {
        self.vec.bump()
    }

    /// Converts a `String` into a byte vector.
    ///
    /// This consumes the `String`, so we do not need to copy its contents.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_str_in("hello", &b);
    ///
    /// assert_eq!(s.into_bytes(), [104, 101, 108, 108, 111]);
    /// ```
    #[inline]
    pub fn into_bytes(self) -> Vec<'bump, u8> {
        self.vec
    }

    /// Convert this `String<'bump>` into a `&'bump str`. This is analogous to
    /// [`std::string::String::into_boxed_str`][into_boxed_str].
    ///
    /// [into_boxed_str]: https://doc.rust-lang.org/std/string/struct.String.html#method.into_boxed_str
    ///
    /// # Example
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_str_in("foo", &b);
    ///
    /// assert_eq!(s.into_bump_str(), "foo");
    /// ```
    pub fn into_bump_str(self) -> &'bump str {
        let s = unsafe {
            let s = self.as_str();
            mem::transmute(s)
        };
        mem::forget(self);
        s
    }

    /// Extracts a string slice containing the entire `String`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_str_in("foo", &b);
    ///
    /// assert_eq!("foo", s.as_str());
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        self
    }

    /// Converts a `String` into a mutable string slice.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foobar", &b);
    /// let s_mut_str = s.as_mut_str();
    ///
    /// s_mut_str.make_ascii_uppercase();
    ///
    /// assert_eq!("FOOBAR", s_mut_str);
    /// ```
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self
    }

    /// Appends a given string slice onto the end of this `String`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foo", &b);
    ///
    /// s.push_str("bar");
    ///
    /// assert_eq!("foobar", s);
    /// ```
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        self.vec.extend_from_slice_copy(string.as_bytes())
    }

    /// Returns this `String`'s capacity, in bytes.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::with_capacity_in(10, &b);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }

    /// Ensures that this `String`'s capacity is at least `additional` bytes
    /// larger than its length.
    ///
    /// The capacity may be increased by more than `additional` bytes if it
    /// chooses, to prevent frequent reallocations.
    ///
    /// If you do not want this "at least" behavior, see the [`reserve_exact`]
    /// method.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows [`usize`].
    ///
    /// [`reserve_exact`]: struct.String.html#method.reserve_exact
    /// [`usize`]: https://doc.rust-lang.org/std/primitive.usize.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::new_in(&b);
    ///
    /// s.reserve(10);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    ///
    /// This may not actually increase the capacity:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::with_capacity_in(10, &b);
    /// s.push('a');
    /// s.push('b');
    ///
    /// // s now has a length of 2 and a capacity of 10
    /// assert_eq!(2, s.len());
    /// assert_eq!(10, s.capacity());
    ///
    /// // Since we already have an extra 8 capacity, calling this...
    /// s.reserve(8);
    ///
    /// // ... doesn't actually increase.
    /// assert_eq!(10, s.capacity());
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.vec.reserve(additional)
    }

    /// Ensures that this `String`'s capacity is `additional` bytes
    /// larger than its length.
    ///
    /// Consider using the [`reserve`] method unless you absolutely know
    /// better than the allocator.
    ///
    /// [`reserve`]: #method.reserve
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::new_in(&b);
    ///
    /// s.reserve_exact(10);
    ///
    /// assert!(s.capacity() >= 10);
    /// ```
    ///
    /// This may not actually increase the capacity:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::with_capacity_in(10, &b);
    /// s.push('a');
    /// s.push('b');
    ///
    /// // s now has a length of 2 and a capacity of 10
    /// assert_eq!(2, s.len());
    /// assert_eq!(10, s.capacity());
    ///
    /// // Since we already have an extra 8 capacity, calling this...
    /// s.reserve_exact(8);
    ///
    /// // ... doesn't actually increase.
    /// assert_eq!(10, s.capacity());
    /// ```
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.vec.reserve_exact(additional)
    }

    /// Shrinks the capacity of this `String` to match its length.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foo", &b);
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to_fit();
    /// assert_eq!(3, s.capacity());
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.vec.shrink_to_fit()
    }

    /// Appends the given [`char`] to the end of this `String`.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("abc", &b);
    ///
    /// s.push('1');
    /// s.push('2');
    /// s.push('3');
    ///
    /// assert_eq!("abc123", s);
    /// ```
    #[inline]
    pub fn push(&mut self, ch: char) {
        match ch.len_utf8() {
            1 => self.vec.push(ch as u8),
            _ => self
                .vec
                .extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }

    /// Returns a byte slice of this `String`'s contents.
    ///
    /// The inverse of this method is [`from_utf8`].
    ///
    /// [`from_utf8`]: #method.from_utf8
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let s = String::from_str_in("hello", &b);
    ///
    /// assert_eq!(&[104, 101, 108, 108, 111], s.as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.vec
    }

    /// Shortens this `String` to the specified length.
    ///
    /// If `new_len` is greater than the string's current length, this has no
    /// effect.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the string.
    ///
    /// # Panics
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("hello", &b);
    ///
    /// s.truncate(2);
    ///
    /// assert_eq!("he", s);
    /// ```
    #[inline]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len <= self.len() {
            assert!(self.is_char_boundary(new_len));
            self.vec.truncate(new_len)
        }
    }

    /// Removes the last character from the string buffer and returns it.
    ///
    /// Returns [`None`] if this `String` is empty.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foo", &b);
    ///
    /// assert_eq!(s.pop(), Some('o'));
    /// assert_eq!(s.pop(), Some('o'));
    /// assert_eq!(s.pop(), Some('f'));
    ///
    /// assert_eq!(s.pop(), None);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        let ch = self.chars().rev().next()?;
        let newlen = self.len() - ch.len_utf8();
        unsafe {
            self.vec.set_len(newlen);
        }
        Some(ch)
    }

    /// Removes a [`char`] from this `String` at a byte position and returns it.
    ///
    /// This is an `O(n)` operation, as it requires copying every element in the
    /// buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than or equal to the `String`'s length,
    /// or if it does not lie on a [`char`] boundary.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foo", &b);
    ///
    /// assert_eq!(s.remove(0), 'f');
    /// assert_eq!(s.remove(1), 'o');
    /// assert_eq!(s.remove(0), 'o');
    /// ```
    #[inline]
    pub fn remove(&mut self, idx: usize) -> char {
        let ch = match self[idx..].chars().next() {
            Some(ch) => ch,
            None => panic!("cannot remove a char from the end of a string"),
        };

        let next = idx + ch.len_utf8();
        let len = self.len();
        unsafe {
            ptr::copy(
                self.vec.as_ptr().add(next),
                self.vec.as_mut_ptr().add(idx),
                len - next,
            );
            self.vec.set_len(len - (next - idx));
        }
        ch
    }

    /// Retains only the characters specified by the predicate.
    ///
    /// In other words, remove all characters `c` such that `f(c)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("f_o_ob_ar", &b);
    ///
    /// s.retain(|c| c != '_');
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(char) -> bool,
    {
        let len = self.len();
        let mut del_bytes = 0;
        let mut idx = 0;

        while idx < len {
            let ch = unsafe { self.get_unchecked(idx..len).chars().next().unwrap() };
            let ch_len = ch.len_utf8();

            if !f(ch) {
                del_bytes += ch_len;
            } else if del_bytes > 0 {
                unsafe {
                    ptr::copy(
                        self.vec.as_ptr().add(idx),
                        self.vec.as_mut_ptr().add(idx - del_bytes),
                        ch_len,
                    );
                }
            }

            // Point idx to the next char
            idx += ch_len;
        }

        if del_bytes > 0 {
            unsafe {
                self.vec.set_len(len - del_bytes);
            }
        }
    }

    /// Inserts a character into this `String` at a byte position.
    ///
    /// This is an `O(n)` operation as it requires copying every element in the
    /// buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than the `String`'s length, or if it does not
    /// lie on a [`char`] boundary.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::with_capacity_in(3, &b);
    ///
    /// s.insert(0, 'f');
    /// s.insert(1, 'o');
    /// s.insert(2, 'o');
    ///
    /// assert_eq!("foo", s);
    /// ```
    #[inline]
    pub fn insert(&mut self, idx: usize, ch: char) {
        assert!(self.is_char_boundary(idx));
        let mut bits = [0; 4];
        let bits = ch.encode_utf8(&mut bits).as_bytes();

        unsafe {
            self.insert_bytes(idx, bits);
        }
    }

    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        let len = self.len();
        let amt = bytes.len();
        self.vec.reserve(amt);

        ptr::copy(
            self.vec.as_ptr().add(idx),
            self.vec.as_mut_ptr().add(idx + amt),
            len - idx,
        );
        ptr::copy(bytes.as_ptr(), self.vec.as_mut_ptr().add(idx), amt);
        self.vec.set_len(len + amt);
    }

    /// Inserts a string slice into this `String` at a byte position.
    ///
    /// This is an `O(n)` operation as it requires copying every element in the
    /// buffer.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is larger than the `String`'s length, or if it does not
    /// lie on a [`char`] boundary.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("bar", &b);
    ///
    /// s.insert_str(0, "foo");
    ///
    /// assert_eq!("foobar", s);
    /// ```
    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        assert!(self.is_char_boundary(idx));

        unsafe {
            self.insert_bytes(idx, string.as_bytes());
        }
    }

    /// Returns a mutable reference to the contents of this `String`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because the returned `&mut Vec` allows writing
    /// bytes which are not valid UTF-8. If this constraint is violated, using
    /// the original `String` after dropping the `&mut Vec` may violate memory
    /// safety, as it is assumed that `String`s are valid UTF-8.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("hello", &b);
    ///
    /// unsafe {
    ///     let vec = s.as_mut_vec();
    ///     assert_eq!(vec, &[104, 101, 108, 108, 111]);
    ///
    ///     vec.reverse();
    /// }
    /// assert_eq!(s, "olleh");
    /// ```
    #[inline]
    pub unsafe fn as_mut_vec(&mut self) -> &mut Vec<'bump, u8> {
        &mut self.vec
    }

    /// Returns the length of this `String`, in bytes.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let a = String::from_str_in("foo", &b);
    ///
    /// assert_eq!(a.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    /// Returns `true` if this `String` has a length of zero.
    ///
    /// Returns `false` otherwise.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut v = String::new_in(&b);
    /// assert!(v.is_empty());
    ///
    /// v.push('a');
    /// assert!(!v.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Splits the string into two at the given index.
    ///
    /// Returns a newly allocated `String`. `self` contains bytes `[0, at)`, and
    /// the returned `String` contains bytes `[at, len)`. `at` must be on the
    /// boundary of a UTF-8 code point.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// # Panics
    ///
    /// Panics if `at` is not on a UTF-8 code point boundary, or if it is beyond the last
    /// code point of the string.
    ///
    /// # Examples
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut hello = String::from_str_in("Hello, World!", &b);
    /// let world = hello.split_off(7);
    /// assert_eq!(hello, "Hello, ");
    /// assert_eq!(world, "World!");
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> String<'bump> {
        assert!(self.is_char_boundary(at));
        let other = self.vec.split_off(at);
        unsafe { String::from_utf8_unchecked(other) }
    }

    /// Truncates this `String`, removing all contents.
    ///
    /// While this means the `String` will have a length of zero, it does not
    /// touch its capacity.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("foo", &b);
    ///
    /// s.clear();
    ///
    /// assert!(s.is_empty());
    /// assert_eq!(0, s.len());
    /// assert_eq!(3, s.capacity());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear()
    }

    /// Creates a draining iterator that removes the specified range in the `String`
    /// and yields the removed `chars`.
    ///
    /// Note: The element range is removed even if the iterator is not
    /// consumed until the end.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("α is alpha, β is beta", &b);
    /// let beta_offset = s.find('β').unwrap_or(s.len());
    ///
    /// // Remove the range up until the β from the string
    /// let t = String::from_iter_in(s.drain(..beta_offset), &b);
    /// assert_eq!(t, "α is alpha, ");
    /// assert_eq!(s, "β is beta");
    ///
    /// // A full range clears the string
    /// drop(s.drain(..));
    /// assert_eq!(s, "");
    /// ```
    pub fn drain<'a, R>(&'a mut self, range: R) -> Drain<'a, 'bump>
    where
        R: RangeBounds<usize>,
    {
        // Memory safety
        //
        // The String version of Drain does not have the memory safety issues
        // of the vector version. The data is just plain bytes.
        // Because the range removal happens in Drop, if the Drain iterator is leaked,
        // the removal will not happen.
        let len = self.len();
        let start = match range.start_bound() {
            Included(&n) => n,
            Excluded(&n) => n + 1,
            Unbounded => 0,
        };
        let end = match range.end_bound() {
            Included(&n) => n + 1,
            Excluded(&n) => n,
            Unbounded => len,
        };

        // Take out two simultaneous borrows. The &mut String won't be accessed
        // until iteration is over, in Drop.
        let self_ptr = self as *mut _;
        // slicing does the appropriate bounds checks
        let chars_iter = self[start..end].chars();

        Drain {
            start,
            end,
            iter: chars_iter,
            string: self_ptr,
        }
    }

    /// Removes the specified range in the string,
    /// and replaces it with the given string.
    /// The given string doesn't need to be the same length as the range.
    ///
    /// # Panics
    ///
    /// Panics if the starting point or end point do not lie on a [`char`]
    /// boundary, or if they're out of bounds.
    ///
    /// [`char`]: https://doc.rust-lang.org/std/primitive.char.html
    /// [`Vec::splice`]: ../vec/struct.Vec.html#method.splice
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// let mut s = String::from_str_in("α is alpha, β is beta", &b);
    /// let beta_offset = s.find('β').unwrap_or(s.len());
    ///
    /// // Replace the range up until the β from the string
    /// s.replace_range(..beta_offset, "Α is capital alpha; ");
    /// assert_eq!(s, "Α is capital alpha; β is beta");
    /// ```
    pub fn replace_range<R>(&mut self, range: R, replace_with: &str)
    where
        R: RangeBounds<usize>,
    {
        // Memory safety
        //
        // Replace_range does not have the memory safety issues of a vector Splice.
        // of the vector version. The data is just plain bytes.

        match range.start_bound() {
            Included(&n) => assert!(self.is_char_boundary(n)),
            Excluded(&n) => assert!(self.is_char_boundary(n + 1)),
            Unbounded => {}
        };
        match range.end_bound() {
            Included(&n) => assert!(self.is_char_boundary(n + 1)),
            Excluded(&n) => assert!(self.is_char_boundary(n)),
            Unbounded => {}
        };

        unsafe { self.as_mut_vec() }.splice(range, replace_with.bytes());
    }
}

impl<'bump> FromUtf8Error<'bump> {
    /// Returns a slice of bytes that were attempted to convert to a `String`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some invalid bytes, in a vector
    /// let bytes = bumpalo::vec![in &b; 0, 159];
    ///
    /// let value = String::from_utf8(bytes);
    ///
    /// assert_eq!(&[0, 159], value.unwrap_err().as_bytes());
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    /// Returns the bytes that were attempted to convert to a `String`.
    ///
    /// This method is carefully constructed to avoid allocation. It will
    /// consume the error, moving out the bytes, so that a copy of the bytes
    /// does not need to be made.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some invalid bytes, in a vector
    /// let bytes = bumpalo::vec![in &b; 0, 159];
    ///
    /// let value = String::from_utf8(bytes);
    ///
    /// assert_eq!(bumpalo::vec![in &b; 0, 159], value.unwrap_err().into_bytes());
    /// ```
    pub fn into_bytes(self) -> Vec<'bump, u8> {
        self.bytes
    }

    /// Fetch a `Utf8Error` to get more details about the conversion failure.
    ///
    /// The [`Utf8Error`] type provided by [`std::str`] represents an error that may
    /// occur when converting a slice of [`u8`]s to a [`&str`]. In this sense, it's
    /// an analogue to `FromUtf8Error`. See its documentation for more details
    /// on using it.
    ///
    /// [`Utf8Error`]: https://doc.rust-lang.org/std/str/struct.Utf8Error.html
    /// [`std::str`]: https://doc.rust-lang.org/std/str/index.html
    /// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
    /// [`&str`]: https://doc.rust-lang.org/std/primitive.str.html
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use bumpalo::{Bump, collections::String};
    ///
    /// let b = Bump::new();
    ///
    /// // some invalid bytes, in a vector
    /// let bytes = bumpalo::vec![in &b; 0, 159];
    ///
    /// let error = String::from_utf8(bytes).unwrap_err().utf8_error();
    ///
    /// // the first byte is invalid here
    /// assert_eq!(1, error.valid_up_to());
    /// ```
    pub fn utf8_error(&self) -> Utf8Error {
        self.error
    }
}

impl<'bump> fmt::Display for FromUtf8Error<'bump> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl fmt::Display for FromUtf16Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt("invalid utf-16: lone surrogate found", f)
    }
}

impl<'bump> Clone for String<'bump> {
    fn clone(&self) -> Self {
        String {
            vec: self.vec.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.vec.clone_from(&source.vec);
    }
}

impl<'bump> Extend<char> for String<'bump> {
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let iterator = iter.into_iter();
        let (lower_bound, _) = iterator.size_hint();
        self.reserve(lower_bound);
        for ch in iterator {
            self.push(ch)
        }
    }
}

impl<'a, 'bump> Extend<&'a char> for String<'bump> {
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }
}

impl<'a, 'bump> Extend<&'a str> for String<'bump> {
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(s)
        }
    }
}

impl<'bump> Extend<String<'bump>> for String<'bump> {
    fn extend<I: IntoIterator<Item = String<'bump>>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s)
        }
    }
}

impl<'bump> Extend<core_alloc::string::String> for String<'bump> {
    fn extend<I: IntoIterator<Item = core_alloc::string::String>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s)
        }
    }
}

impl<'a, 'bump> Extend<Cow<'a, str>> for String<'bump> {
    fn extend<I: IntoIterator<Item = Cow<'a, str>>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s)
        }
    }
}

impl<'bump> PartialEq for String<'bump> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        PartialEq::eq(&self[..], &other[..])
    }
}

macro_rules! impl_eq {
    ($lhs:ty, $rhs: ty) => {
        impl<'a, 'bump> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                PartialEq::eq(&self[..], &other[..])
            }
        }

        impl<'a, 'b, 'bump> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                PartialEq::eq(&self[..], &other[..])
            }
        }
    };
}

impl_eq! { String<'bump>, str }
impl_eq! { String<'bump>, &'a str }
impl_eq! { Cow<'a, str>, String<'bump> }
impl_eq! { core_alloc::string::String, String<'bump> }

impl<'bump> fmt::Display for String<'bump> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'bump> fmt::Debug for String<'bump> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'bump> hash::Hash for String<'bump> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        (**self).hash(hasher)
    }
}

/// Implements the `+` operator for concatenating two strings.
///
/// This consumes the `String<'bump>` on the left-hand side and re-uses its buffer (growing it if
/// necessary). This is done to avoid allocating a new `String<'bump>` and copying the entire contents on
/// every operation, which would lead to `O(n^2)` running time when building an `n`-byte string by
/// repeated concatenation.
///
/// The string on the right-hand side is only borrowed; its contents are copied into the returned
/// `String<'bump>`.
///
/// # Examples
///
/// Concatenating two `String<'bump>`s takes the first by value and borrows the second:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let bump = Bump::new();
///
/// let a = String::from_str_in("hello", &bump);
/// let b = String::from_str_in(" world", &bump);
/// let c = a + &b;
/// // `a` is moved and can no longer be used here.
/// ```
///
/// If you want to keep using the first `String`, you can clone it and append to the clone instead:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let bump = Bump::new();
///
/// let a = String::from_str_in("hello", &bump);
/// let b = String::from_str_in(" world", &bump);
/// let c = a.clone() + &b;
/// // `a` is still valid here.
/// ```
///
/// Concatenating `&str` slices can be done by converting the first to a `String`:
///
/// ```
/// use bumpalo::{Bump, collections::String};
///
/// let bump = Bump::new();
///
/// let a = "hello";
/// let b = " world";
/// let c = String::from_str_in(a, &bump) + b;
/// ```
impl<'a, 'bump> Add<&'a str> for String<'bump> {
    type Output = String<'bump>;

    #[inline]
    fn add(mut self, other: &str) -> String<'bump> {
        self.push_str(other);
        self
    }
}

/// Implements the `+=` operator for appending to a `String<'bump>`.
///
/// This has the same behavior as the [`push_str`][String::push_str] method.
impl<'a, 'bump> AddAssign<&'a str> for String<'bump> {
    #[inline]
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}

impl<'bump> ops::Index<ops::Range<usize>> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::Range<usize>) -> &str {
        &self[..][index]
    }
}
impl<'bump> ops::Index<ops::RangeTo<usize>> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeTo<usize>) -> &str {
        &self[..][index]
    }
}
impl<'bump> ops::Index<ops::RangeFrom<usize>> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &str {
        &self[..][index]
    }
}
impl<'bump> ops::Index<ops::RangeFull> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &str {
        unsafe { str::from_utf8_unchecked(&self.vec) }
    }
}
impl<'bump> ops::Index<ops::RangeInclusive<usize>> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}
impl<'bump> ops::Index<ops::RangeToInclusive<usize>> for String<'bump> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeToInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}

impl<'bump> ops::IndexMut<ops::Range<usize>> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, index: ops::Range<usize>) -> &mut str {
        &mut self[..][index]
    }
}
impl<'bump> ops::IndexMut<ops::RangeTo<usize>> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, index: ops::RangeTo<usize>) -> &mut str {
        &mut self[..][index]
    }
}
impl<'bump> ops::IndexMut<ops::RangeFrom<usize>> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, index: ops::RangeFrom<usize>) -> &mut str {
        &mut self[..][index]
    }
}
impl<'bump> ops::IndexMut<ops::RangeFull> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, _index: ops::RangeFull) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(&mut *self.vec) }
    }
}
impl<'bump> ops::IndexMut<ops::RangeInclusive<usize>> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, index: ops::RangeInclusive<usize>) -> &mut str {
        IndexMut::index_mut(&mut **self, index)
    }
}
impl<'bump> ops::IndexMut<ops::RangeToInclusive<usize>> for String<'bump> {
    #[inline]
    fn index_mut(&mut self, index: ops::RangeToInclusive<usize>) -> &mut str {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl<'bump> ops::Deref for String<'bump> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.vec) }
    }
}

impl<'bump> ops::DerefMut for String<'bump> {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(&mut *self.vec) }
    }
}

impl<'bump> AsRef<str> for String<'bump> {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}

impl<'bump> AsRef<[u8]> for String<'bump> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'bump> fmt::Write for String<'bump> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl<'bump> Borrow<str> for String<'bump> {
    #[inline]
    fn borrow(&self) -> &str {
        &self[..]
    }
}

impl<'bump> BorrowMut<str> for String<'bump> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut str {
        &mut self[..]
    }
}

/// A draining iterator for `String`.
///
/// This struct is created by the [`String::drain`] method. See its
/// documentation for more information.
pub struct Drain<'a, 'bump> {
    /// Will be used as &'a mut String in the destructor
    string: *mut String<'bump>,
    /// Start of part to remove
    start: usize,
    /// End of part to remove
    end: usize,
    /// Current remaining range to remove
    iter: Chars<'a>,
}

impl<'a, 'bump> fmt::Debug for Drain<'a, 'bump> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Drain { .. }")
    }
}

unsafe impl<'a, 'bump> Sync for Drain<'a, 'bump> {}
unsafe impl<'a, 'bump> Send for Drain<'a, 'bump> {}

impl<'a, 'bump> Drop for Drain<'a, 'bump> {
    fn drop(&mut self) {
        unsafe {
            // Use Vec::drain. "Reaffirm" the bounds checks to avoid
            // panic code being inserted again.
            let self_vec = (*self.string).as_mut_vec();
            if self.start <= self.end && self.end <= self_vec.len() {
                self_vec.drain(self.start..self.end);
            }
        }
    }
}

// TODO: implement `AsRef<str/[u8]>` and `as_str`

impl<'a, 'bump> Iterator for Drain<'a, 'bump> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, 'bump> DoubleEndedIterator for Drain<'a, 'bump> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        self.iter.next_back()
    }
}

impl<'a, 'bump> FusedIterator for Drain<'a, 'bump> {}

#[cfg(feature = "serde")]
mod serialize {
    use super::*;

    use serde::{Serialize, Serializer};

    impl<'bump> Serialize for String<'bump> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self)
        }
    }
}
