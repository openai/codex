// Copyright 2013-2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! URLs use special characters to indicate the parts of the request.
//! For example, a `?` question mark marks the end of a path and the start of a query string.
//! In order for that character to exist inside a path, it needs to be encoded differently.
//!
//! Percent encoding replaces reserved characters with the `%` escape character
//! followed by a byte value as two hexadecimal digits.
//! For example, an ASCII space is replaced with `%20`.
//!
//! When encoding, the set of characters that can (and should, for readability) be left alone
//! depends on the context.
//! The `?` question mark mentioned above is not a separator when used literally
//! inside of a query string, and therefore does not need to be encoded.
//! The [`AsciiSet`] parameter of [`percent_encode`] and [`utf8_percent_encode`]
//! lets callers configure this.
//!
//! This crate deliberately does not provide many different sets.
//! Users should consider in what context the encoded string will be used,
//! read relevant specifications, and define their own set.
//! This is done by using the `add` method of an existing set.
//!
//! # Examples
//!
//! ```
//! use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
//!
//! /// https://url.spec.whatwg.org/#fragment-percent-encode-set
//! const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
//!
//! assert_eq!(utf8_percent_encode("foo <bar>", FRAGMENT).to_string(), "foo%20%3Cbar%3E");
//! ```
#![no_std]

// For forwards compatibility
#[cfg(feature = "std")]
extern crate std as _;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::{
    borrow::{Cow, ToOwned},
    string::String,
    vec::Vec,
};
use core::{fmt, mem, slice, str};

/// Represents a set of characters or bytes in the ASCII range.
///
/// This is used in [`percent_encode`] and [`utf8_percent_encode`].
/// This is similar to [percent-encode sets](https://url.spec.whatwg.org/#percent-encoded-bytes).
///
/// Use the `add` method of an existing set to define a new set. For example:
///
/// ```
/// use percent_encoding::{AsciiSet, CONTROLS};
///
/// /// https://url.spec.whatwg.org/#fragment-percent-encode-set
/// const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
/// ```
pub struct AsciiSet {
    mask: [Chunk; ASCII_RANGE_LEN / BITS_PER_CHUNK],
}

type Chunk = u32;

const ASCII_RANGE_LEN: usize = 0x80;

const BITS_PER_CHUNK: usize = 8 * mem::size_of::<Chunk>();

impl AsciiSet {
    /// Called with UTF-8 bytes rather than code points.
    /// Not used for non-ASCII bytes.
    const fn contains(&self, byte: u8) -> bool {
        let chunk = self.mask[byte as usize / BITS_PER_CHUNK];
        let mask = 1 << (byte as usize % BITS_PER_CHUNK);
        (chunk & mask) != 0
    }

    fn should_percent_encode(&self, byte: u8) -> bool {
        !byte.is_ascii() || self.contains(byte)
    }

    pub const fn add(&self, byte: u8) -> Self {
        let mut mask = self.mask;
        mask[byte as usize / BITS_PER_CHUNK] |= 1 << (byte as usize % BITS_PER_CHUNK);
        AsciiSet { mask }
    }

    pub const fn remove(&self, byte: u8) -> Self {
        let mut mask = self.mask;
        mask[byte as usize / BITS_PER_CHUNK] &= !(1 << (byte as usize % BITS_PER_CHUNK));
        AsciiSet { mask }
    }
}

/// The set of 0x00 to 0x1F (C0 controls), and 0x7F (DEL).
///
/// Note that this includes the newline and tab characters, but not the space 0x20.
///
/// <https://url.spec.whatwg.org/#c0-control-percent-encode-set>
pub const CONTROLS: &AsciiSet = &AsciiSet {
    mask: [
        !0_u32, // C0: 0x00 to 0x1F (32 bits set)
        0,
        0,
        1 << (0x7F_u32 % 32), // DEL: 0x7F (one bit set)
    ],
};

macro_rules! static_assert {
    ($( $bool: expr, )+) => {
        fn _static_assert() {
            $(
                let _ = mem::transmute::<[u8; $bool as usize], u8>;
            )+
        }
    }
}

static_assert! {
    CONTROLS.contains(0x00),
    CONTROLS.contains(0x1F),
    !CONTROLS.contains(0x20),
    !CONTROLS.contains(0x7E),
    CONTROLS.contains(0x7F),
}

/// Everything that is not an ASCII letter or digit.
///
/// This is probably more eager than necessary in any context.
pub const NON_ALPHANUMERIC: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'-')
    .add(b'.')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'_')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'~');

/// Return the percent-encoding of the given byte.
///
/// This is unconditional, unlike `percent_encode()` which has an `AsciiSet` parameter.
///
/// # Examples
///
/// ```
/// use percent_encoding::percent_encode_byte;
///
/// assert_eq!("foo bar".bytes().map(percent_encode_byte).collect::<String>(),
///            "%66%6F%6F%20%62%61%72");
/// ```
#[inline]
pub fn percent_encode_byte(byte: u8) -> &'static str {
    static ENC_TABLE: &[u8; 768] = b"\
      %00%01%02%03%04%05%06%07%08%09%0A%0B%0C%0D%0E%0F\
      %10%11%12%13%14%15%16%17%18%19%1A%1B%1C%1D%1E%1F\
      %20%21%22%23%24%25%26%27%28%29%2A%2B%2C%2D%2E%2F\
      %30%31%32%33%34%35%36%37%38%39%3A%3B%3C%3D%3E%3F\
      %40%41%42%43%44%45%46%47%48%49%4A%4B%4C%4D%4E%4F\
      %50%51%52%53%54%55%56%57%58%59%5A%5B%5C%5D%5E%5F\
      %60%61%62%63%64%65%66%67%68%69%6A%6B%6C%6D%6E%6F\
      %70%71%72%73%74%75%76%77%78%79%7A%7B%7C%7D%7E%7F\
      %80%81%82%83%84%85%86%87%88%89%8A%8B%8C%8D%8E%8F\
      %90%91%92%93%94%95%96%97%98%99%9A%9B%9C%9D%9E%9F\
      %A0%A1%A2%A3%A4%A5%A6%A7%A8%A9%AA%AB%AC%AD%AE%AF\
      %B0%B1%B2%B3%B4%B5%B6%B7%B8%B9%BA%BB%BC%BD%BE%BF\
      %C0%C1%C2%C3%C4%C5%C6%C7%C8%C9%CA%CB%CC%CD%CE%CF\
      %D0%D1%D2%D3%D4%D5%D6%D7%D8%D9%DA%DB%DC%DD%DE%DF\
      %E0%E1%E2%E3%E4%E5%E6%E7%E8%E9%EA%EB%EC%ED%EE%EF\
      %F0%F1%F2%F3%F4%F5%F6%F7%F8%F9%FA%FB%FC%FD%FE%FF\
      ";

    let index = usize::from(byte) * 3;
    // SAFETY: ENC_TABLE is ascii-only, so any subset if it should be
    // ascii-only too, which is valid utf8.
    unsafe { str::from_utf8_unchecked(&ENC_TABLE[index..index + 3]) }
}

/// Percent-encode the given bytes with the given set.
///
/// Non-ASCII bytes and bytes in `ascii_set` are encoded.
///
/// The return type:
///
/// * Implements `Iterator<Item = &str>` and therefore has a `.collect::<String>()` method,
/// * Implements `Display` and therefore has a `.to_string()` method,
/// * Implements `Into<Cow<str>>` borrowing `input` when none of its bytes are encoded.
///
/// # Examples
///
/// ```
/// use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
///
/// assert_eq!(percent_encode(b"foo bar?", NON_ALPHANUMERIC).to_string(), "foo%20bar%3F");
/// ```
#[inline]
pub fn percent_encode<'a>(input: &'a [u8], ascii_set: &'static AsciiSet) -> PercentEncode<'a> {
    PercentEncode {
        bytes: input,
        ascii_set,
    }
}

/// Percent-encode the UTF-8 encoding of the given string.
///
/// See [`percent_encode`] regarding the return type.
///
/// # Examples
///
/// ```
/// use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
///
/// assert_eq!(utf8_percent_encode("foo bar?", NON_ALPHANUMERIC).to_string(), "foo%20bar%3F");
/// ```
#[inline]
pub fn utf8_percent_encode<'a>(input: &'a str, ascii_set: &'static AsciiSet) -> PercentEncode<'a> {
    percent_encode(input.as_bytes(), ascii_set)
}

/// The return type of [`percent_encode`] and [`utf8_percent_encode`].
#[derive(Clone)]
pub struct PercentEncode<'a> {
    bytes: &'a [u8],
    ascii_set: &'static AsciiSet,
}

impl<'a> Iterator for PercentEncode<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if let Some((&first_byte, remaining)) = self.bytes.split_first() {
            if self.ascii_set.should_percent_encode(first_byte) {
                self.bytes = remaining;
                Some(percent_encode_byte(first_byte))
            } else {
                // The unsafe blocks here are appropriate because the bytes are
                // confirmed as a subset of UTF-8 in should_percent_encode.
                for (i, &byte) in remaining.iter().enumerate() {
                    if self.ascii_set.should_percent_encode(byte) {
                        // 1 for first_byte + i for previous iterations of this loop
                        let (unchanged_slice, remaining) = self.bytes.split_at(1 + i);
                        self.bytes = remaining;
                        return Some(unsafe { str::from_utf8_unchecked(unchanged_slice) });
                    }
                }
                let unchanged_slice = self.bytes;
                self.bytes = &[][..];
                Some(unsafe { str::from_utf8_unchecked(unchanged_slice) })
            }
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

impl<'a> fmt::Display for PercentEncode<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in (*self).clone() {
            formatter.write_str(c)?
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<PercentEncode<'a>> for Cow<'a, str> {
    fn from(mut iter: PercentEncode<'a>) -> Self {
        match iter.next() {
            None => "".into(),
            Some(first) => match iter.next() {
                None => first.into(),
                Some(second) => {
                    let mut string = first.to_owned();
                    string.push_str(second);
                    string.extend(iter);
                    string.into()
                }
            },
        }
    }
}

/// Percent-decode the given string.
///
/// <https://url.spec.whatwg.org/#string-percent-decode>
///
/// See [`percent_decode`] regarding the return type.
#[inline]
pub fn percent_decode_str(input: &str) -> PercentDecode<'_> {
    percent_decode(input.as_bytes())
}

/// Percent-decode the given bytes.
///
/// <https://url.spec.whatwg.org/#percent-decode>
///
/// Any sequence of `%` followed by two hexadecimal digits is decoded.
/// The return type:
///
/// * Implements `Into<Cow<u8>>` borrowing `input` when it contains no percent-encoded sequence,
/// * Implements `Iterator<Item = u8>` and therefore has a `.collect::<Vec<u8>>()` method,
/// * Has `decode_utf8()` and `decode_utf8_lossy()` methods.
///
/// # Examples
///
/// ```
/// use percent_encoding::percent_decode;
///
/// assert_eq!(percent_decode(b"foo%20bar%3f").decode_utf8().unwrap(), "foo bar?");
/// ```
#[inline]
pub fn percent_decode(input: &[u8]) -> PercentDecode<'_> {
    PercentDecode {
        bytes: input.iter(),
    }
}

/// The return type of [`percent_decode`].
#[derive(Clone, Debug)]
pub struct PercentDecode<'a> {
    bytes: slice::Iter<'a, u8>,
}

fn after_percent_sign(iter: &mut slice::Iter<'_, u8>) -> Option<u8> {
    let mut cloned_iter = iter.clone();
    let h = char::from(*cloned_iter.next()?).to_digit(16)?;
    let l = char::from(*cloned_iter.next()?).to_digit(16)?;
    *iter = cloned_iter;
    Some(h as u8 * 0x10 + l as u8)
}

impl<'a> Iterator for PercentDecode<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        self.bytes.next().map(|&byte| {
            if byte == b'%' {
                after_percent_sign(&mut self.bytes).unwrap_or(byte)
            } else {
                byte
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let bytes = self.bytes.len();
        ((bytes + 2) / 3, Some(bytes))
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<PercentDecode<'a>> for Cow<'a, [u8]> {
    fn from(iter: PercentDecode<'a>) -> Self {
        match iter.if_any() {
            Some(vec) => Cow::Owned(vec),
            None => Cow::Borrowed(iter.bytes.as_slice()),
        }
    }
}

impl<'a> PercentDecode<'a> {
    /// If the percent-decoding is different from the input, return it as a new bytes vector.
    #[cfg(feature = "alloc")]
    fn if_any(&self) -> Option<Vec<u8>> {
        let mut bytes_iter = self.bytes.clone();
        while bytes_iter.any(|&b| b == b'%') {
            if let Some(decoded_byte) = after_percent_sign(&mut bytes_iter) {
                let initial_bytes = self.bytes.as_slice();
                let unchanged_bytes_len = initial_bytes.len() - bytes_iter.len() - 3;
                let mut decoded = initial_bytes[..unchanged_bytes_len].to_owned();
                decoded.push(decoded_byte);
                decoded.extend(PercentDecode { bytes: bytes_iter });
                return Some(decoded);
            }
        }
        // Nothing to decode
        None
    }

    /// Decode the result of percent-decoding as UTF-8.
    ///
    /// This is return `Err` when the percent-decoded bytes are not well-formed in UTF-8.
    #[cfg(feature = "alloc")]
    pub fn decode_utf8(self) -> Result<Cow<'a, str>, str::Utf8Error> {
        match self.clone().into() {
            Cow::Borrowed(bytes) => match str::from_utf8(bytes) {
                Ok(s) => Ok(s.into()),
                Err(e) => Err(e),
            },
            Cow::Owned(bytes) => match String::from_utf8(bytes) {
                Ok(s) => Ok(s.into()),
                Err(e) => Err(e.utf8_error()),
            },
        }
    }

    /// Decode the result of percent-decoding as UTF-8, lossily.
    ///
    /// Invalid UTF-8 percent-encoded byte sequences will be replaced � U+FFFD,
    /// the replacement character.
    #[cfg(feature = "alloc")]
    pub fn decode_utf8_lossy(self) -> Cow<'a, str> {
        decode_utf8_lossy(self.clone().into())
    }
}

#[cfg(feature = "alloc")]
fn decode_utf8_lossy(input: Cow<'_, [u8]>) -> Cow<'_, str> {
    // Note: This function is duplicated in `form_urlencoded/src/query_encoding.rs`.
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
