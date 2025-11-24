// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Parser and serializer for the [`application/x-www-form-urlencoded` syntax](
//! http://url.spec.whatwg.org/#application/x-www-form-urlencoded),
//! as used by HTML forms.
//!
//! Converts between a string (such as an URL’s query string)
//! and a sequence of (name, value) pairs.
#![no_std]

// For forwards compatibility
#[cfg(feature = "std")]
extern crate std as _;

extern crate alloc;

#[cfg(not(feature = "alloc"))]
compile_error!("the `alloc` feature must currently be enabled");

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::string::String;
use core::str;
use percent_encoding::{percent_decode, percent_encode_byte};

/// Convert a byte string in the `application/x-www-form-urlencoded` syntax
/// into a iterator of (name, value) pairs.
///
/// Use `parse(input.as_bytes())` to parse a `&str` string.
///
/// The names and values are percent-decoded. For instance, `%23first=%25try%25` will be
/// converted to `[("#first", "%try%")]`.
#[inline]
pub fn parse(input: &[u8]) -> Parse<'_> {
    Parse { input }
}
/// The return type of `parse()`.
#[derive(Copy, Clone)]
pub struct Parse<'a> {
    input: &'a [u8],
}

impl<'a> Iterator for Parse<'a> {
    type Item = (Cow<'a, str>, Cow<'a, str>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.input.is_empty() {
                return None;
            }
            let mut split2 = self.input.splitn(2, |&b| b == b'&');
            let sequence = split2.next().unwrap();
            self.input = split2.next().unwrap_or(&[][..]);
            if sequence.is_empty() {
                continue;
            }
            let mut split2 = sequence.splitn(2, |&b| b == b'=');
            let name = split2.next().unwrap();
            let value = split2.next().unwrap_or(&[][..]);
            return Some((decode(name), decode(value)));
        }
    }
}

fn decode(input: &[u8]) -> Cow<'_, str> {
    let replaced = replace_plus(input);
    decode_utf8_lossy(match percent_decode(&replaced).into() {
        Cow::Owned(vec) => Cow::Owned(vec),
        Cow::Borrowed(_) => replaced,
    })
}

/// Replace b'+' with b' '
fn replace_plus(input: &[u8]) -> Cow<'_, [u8]> {
    match input.iter().position(|&b| b == b'+') {
        None => Cow::Borrowed(input),
        Some(first_position) => {
            let mut replaced = input.to_owned();
            replaced[first_position] = b' ';
            for byte in &mut replaced[first_position + 1..] {
                if *byte == b'+' {
                    *byte = b' ';
                }
            }
            Cow::Owned(replaced)
        }
    }
}

impl<'a> Parse<'a> {
    /// Return a new iterator that yields pairs of `String` instead of pairs of `Cow<str>`.
    pub fn into_owned(self) -> ParseIntoOwned<'a> {
        ParseIntoOwned { inner: self }
    }
}

/// Like `Parse`, but yields pairs of `String` instead of pairs of `Cow<str>`.
pub struct ParseIntoOwned<'a> {
    inner: Parse<'a>,
}

impl<'a> Iterator for ParseIntoOwned<'a> {
    type Item = (String, String);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
    }
}

/// The [`application/x-www-form-urlencoded` byte serializer](
/// https://url.spec.whatwg.org/#concept-urlencoded-byte-serializer).
///
/// Return an iterator of `&str` slices.
pub fn byte_serialize(input: &[u8]) -> ByteSerialize<'_> {
    ByteSerialize { bytes: input }
}

/// Return value of `byte_serialize()`.
#[derive(Debug)]
pub struct ByteSerialize<'a> {
    bytes: &'a [u8],
}

fn byte_serialized_unchanged(byte: u8) -> bool {
    matches!(byte, b'*' | b'-' | b'.' | b'0' ..= b'9' | b'A' ..= b'Z' | b'_' | b'a' ..= b'z')
}

impl<'a> Iterator for ByteSerialize<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if let Some((&first, tail)) = self.bytes.split_first() {
            if !byte_serialized_unchanged(first) {
                self.bytes = tail;
                return Some(if first == b' ' {
                    "+"
                } else {
                    percent_encode_byte(first)
                });
            }
            let position = tail.iter().position(|&b| !byte_serialized_unchanged(b));
            let (unchanged_slice, remaining) = match position {
                // 1 for first_byte + i unchanged in tail
                Some(i) => self.bytes.split_at(1 + i),
                None => (self.bytes, &[][..]),
            };
            self.bytes = remaining;
            // This unsafe is appropriate because we have already checked these
            // bytes in byte_serialized_unchanged, which checks for a subset
            // of UTF-8. So we know these bytes are valid UTF-8, and doing
            // another UTF-8 check would be wasteful.
            Some(unsafe { str::from_utf8_unchecked(unchanged_slice) })
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.bytes.is_empty() {
            (0, Some(0))
        } else {
            (1, Some(self.bytes.len()))
        }
    }
}

/// The [`application/x-www-form-urlencoded` serializer](
/// https://url.spec.whatwg.org/#concept-urlencoded-serializer).
pub struct Serializer<'a, T: Target> {
    target: Option<T>,
    start_position: usize,
    encoding: EncodingOverride<'a>,
}

pub trait Target {
    fn as_mut_string(&mut self) -> &mut String;
    fn finish(self) -> Self::Finished;
    type Finished;
}

impl Target for String {
    fn as_mut_string(&mut self) -> &mut String {
        self
    }
    fn finish(self) -> Self {
        self
    }
    type Finished = Self;
}

impl<'a> Target for &'a mut String {
    fn as_mut_string(&mut self) -> &mut String {
        self
    }
    fn finish(self) -> Self {
        self
    }
    type Finished = Self;
}

impl<'a, T: Target> Serializer<'a, T> {
    /// Create a new `application/x-www-form-urlencoded` serializer for the given target.
    ///
    /// If the target is non-empty,
    /// its content is assumed to already be in `application/x-www-form-urlencoded` syntax.
    pub fn new(target: T) -> Self {
        Self::for_suffix(target, 0)
    }

    /// Create a new `application/x-www-form-urlencoded` serializer
    /// for a suffix of the given target.
    ///
    /// If that suffix is non-empty,
    /// its content is assumed to already be in `application/x-www-form-urlencoded` syntax.
    pub fn for_suffix(mut target: T, start_position: usize) -> Self {
        if target.as_mut_string().len() < start_position {
            panic!(
                "invalid length {} for target of length {}",
                start_position,
                target.as_mut_string().len()
            );
        }

        Serializer {
            target: Some(target),
            start_position,
            encoding: None,
        }
    }

    /// Remove any existing name/value pair.
    ///
    /// Panics if called after `.finish()`.
    pub fn clear(&mut self) -> &mut Self {
        string(&mut self.target).truncate(self.start_position);
        self
    }

    /// Set the character encoding to be used for names and values before percent-encoding.
    pub fn encoding_override(&mut self, new: EncodingOverride<'a>) -> &mut Self {
        self.encoding = new;
        self
    }

    /// Serialize and append a name/value pair.
    ///
    /// Panics if called after `.finish()`.
    pub fn append_pair(&mut self, name: &str, value: &str) -> &mut Self {
        append_pair(
            string(&mut self.target),
            self.start_position,
            self.encoding,
            name,
            value,
        );
        self
    }

    /// Serialize and append a name of parameter without any value.
    ///
    /// Panics if called after `.finish()`.
    pub fn append_key_only(&mut self, name: &str) -> &mut Self {
        append_key_only(
            string(&mut self.target),
            self.start_position,
            self.encoding,
            name,
        );
        self
    }

    /// Serialize and append a number of name/value pairs.
    ///
    /// This simply calls `append_pair` repeatedly.
    /// This can be more convenient, so the user doesn’t need to introduce a block
    /// to limit the scope of `Serializer`’s borrow of its string.
    ///
    /// Panics if called after `.finish()`.
    pub fn extend_pairs<I, K, V>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        {
            let string = string(&mut self.target);
            for pair in iter {
                let (k, v) = pair.borrow();
                append_pair(
                    string,
                    self.start_position,
                    self.encoding,
                    k.as_ref(),
                    v.as_ref(),
                );
            }
        }
        self
    }

    /// Serialize and append a number of names without values.
    ///
    /// This simply calls `append_key_only` repeatedly.
    /// This can be more convenient, so the user doesn’t need to introduce a block
    /// to limit the scope of `Serializer`’s borrow of its string.
    ///
    /// Panics if called after `.finish()`.
    pub fn extend_keys_only<I, K>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: Borrow<K>,
        K: AsRef<str>,
    {
        {
            let string = string(&mut self.target);
            for key in iter {
                let k = key.borrow().as_ref();
                append_key_only(string, self.start_position, self.encoding, k);
            }
        }
        self
    }

    /// If this serializer was constructed with a string, take and return that string.
    ///
    /// ```rust
    /// use form_urlencoded;
    /// let encoded: String = form_urlencoded::Serializer::new(String::new())
    ///     .append_pair("foo", "bar & baz")
    ///     .append_pair("saison", "Été+hiver")
    ///     .finish();
    /// assert_eq!(encoded, "foo=bar+%26+baz&saison=%C3%89t%C3%A9%2Bhiver");
    /// ```
    ///
    /// Panics if called more than once.
    pub fn finish(&mut self) -> T::Finished {
        self.target
            .take()
            .expect("url::form_urlencoded::Serializer double finish")
            .finish()
    }
}

fn append_separator_if_needed(string: &mut String, start_position: usize) {
    if string.len() > start_position {
        string.push('&')
    }
}

fn string<T: Target>(target: &mut Option<T>) -> &mut String {
    target
        .as_mut()
        .expect("url::form_urlencoded::Serializer finished")
        .as_mut_string()
}

fn append_pair(
    string: &mut String,
    start_position: usize,
    encoding: EncodingOverride<'_>,
    name: &str,
    value: &str,
) {
    append_separator_if_needed(string, start_position);
    append_encoded(name, string, encoding);
    string.push('=');
    append_encoded(value, string, encoding);
}

fn append_key_only(
    string: &mut String,
    start_position: usize,
    encoding: EncodingOverride,
    name: &str,
) {
    append_separator_if_needed(string, start_position);
    append_encoded(name, string, encoding);
}

fn append_encoded(s: &str, string: &mut String, encoding: EncodingOverride<'_>) {
    string.extend(byte_serialize(&encode(encoding, s)))
}

pub(crate) fn encode<'a>(encoding_override: EncodingOverride<'_>, input: &'a str) -> Cow<'a, [u8]> {
    if let Some(o) = encoding_override {
        return o(input);
    }
    input.as_bytes().into()
}

pub(crate) fn decode_utf8_lossy(input: Cow<'_, [u8]>) -> Cow<'_, str> {
    // Note: This function is duplicated in `percent_encoding/lib.rs`.
    match input {
        Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
        Cow::Owned(bytes) => {
            match String::from_utf8_lossy(&bytes) {
                Cow::Borrowed(utf8) => {
                    // If from_utf8_lossy returns a Cow::Borrowed, then we can
                    // be sure our original bytes were valid UTF-8. This is because
                    // if the bytes were invalid UTF-8 from_utf8_lossy would have
                    // to allocate a new owned string to back the Cow so it could
                    // replace invalid bytes with a placeholder.

                    // First we do a debug_assert to confirm our description above.
                    let raw_utf8: *const [u8] = utf8.as_bytes();
                    debug_assert!(raw_utf8 == &*bytes as *const [u8]);

                    // Given we know the original input bytes are valid UTF-8,
                    // and we have ownership of those bytes, we re-use them and
                    // return a Cow::Owned here.
                    Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) })
                }
                Cow::Owned(s) => Cow::Owned(s),
            }
        }
    }
}

pub type EncodingOverride<'a> = Option<&'a dyn Fn(&str) -> Cow<'_, [u8]>>;
