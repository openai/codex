// Copyright 2015 Nicholas Allegra (comex).
// Licensed under the Apache License, Version 2.0 <https://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license <https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! Parse strings like, and escape strings for, POSIX shells.
//!
//! Same idea as (but implementation not directly based on) the Python shlex module.
//!
//! Disabling the `std` feature (which is enabled by default) will allow the crate to work in
//! `no_std` environments, where the `alloc` crate, and a global allocator, are available.
//!
//! ## <span style="color:red">Warning</span>
//!
//! The [`try_quote`]/[`try_join`] family of APIs does not quote control characters (because they
//! cannot be quoted portably).
//!
//! This is fully safe in noninteractive contexts, like shell scripts and `sh -c` arguments (or
//! even scripts `source`d from interactive shells).
//!
//! But if you are quoting for human consumption, you should keep in mind that ugly inputs produce
//! ugly outputs (which may not be copy-pastable).
//!
//! And if by chance you are piping the output of [`try_quote`]/[`try_join`] directly to the stdin
//! of an interactive shell, you should stop, because control characters can lead to arbitrary
//! command injection.
//!
//! For more information, and for information about more minor issues, please see [quoting_warning].
//!
//! ## Compatibility
//!
//! This crate's quoting functionality tries to be compatible with **any POSIX-compatible shell**;
//! it's tested against `bash`, `zsh`, `dash`, Busybox `ash`, and `mksh`, plus `fish` (which is not
//! POSIX-compatible but close enough).
//!
//! It also aims to be compatible with Python `shlex` and C `wordexp`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::borrow::Cow;
use alloc::string::String;
#[cfg(test)]
use alloc::vec;
#[cfg(test)]
use alloc::borrow::ToOwned;

pub mod bytes;
#[cfg(all(doc, not(doctest)))]
#[path = "quoting_warning.md"]
pub mod quoting_warning;

/// An iterator that takes an input string and splits it into the words using the same syntax as
/// the POSIX shell.
///
/// See [`bytes::Shlex`].
pub struct Shlex<'a>(bytes::Shlex<'a>);

impl<'a> Shlex<'a> {
    pub fn new(in_str: &'a str) -> Self {
        Self(bytes::Shlex::new(in_str.as_bytes()))
    }
}

impl<'a> Iterator for Shlex<'a> {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        self.0.next().map(|byte_word| {
            // Safety: given valid UTF-8, bytes::Shlex will always return valid UTF-8.
            unsafe { String::from_utf8_unchecked(byte_word) }
        })
    }
}

impl<'a> core::ops::Deref for Shlex<'a> {
    type Target = bytes::Shlex<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> core::ops::DerefMut for Shlex<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Convenience function that consumes the whole string at once.  Returns None if the input was
/// erroneous.
pub fn split(in_str: &str) -> Option<Vec<String>> {
    let mut shl = Shlex::new(in_str);
    let res = shl.by_ref().collect();
    if shl.had_error { None } else { Some(res) }
}

/// Errors from [`Quoter::quote`], [`Quoter::join`], etc. (and their [`bytes`] counterparts).
///
/// By default, the only error that can be returned is [`QuoteError::Nul`].  If you call
/// `allow_nul(true)`, then no errors can be returned at all.  Any error variants added in the
/// future will not be enabled by default; they will be enabled through corresponding non-default
/// [`Quoter`] options.
///
/// ...In theory.  In the unlikely event that additional classes of inputs are discovered that,
/// like nul bytes, are fundamentally unsafe to quote even for non-interactive shells, the risk
/// will be mitigated by adding corresponding [`QuoteError`] variants that *are* enabled by
/// default.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuoteError {
    /// The input contained a nul byte.  In most cases, shells fundamentally [cannot handle strings
    /// containing nul bytes](quoting_warning#nul-bytes), no matter how they are quoted.  But if
    /// you're sure you can handle nul bytes, you can call `allow_nul(true)` on the `Quoter` to let
    /// them pass through.
    Nul,
}

impl core::fmt::Display for QuoteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QuoteError::Nul => f.write_str("cannot shell-quote string containing nul byte"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for QuoteError {}

/// A more configurable interface to quote strings.  If you only want the default settings you can
/// use the convenience functions [`try_quote`] and [`try_join`].
///
/// The bytes equivalent is [`bytes::Quoter`].
#[derive(Default, Debug, Clone)]
pub struct Quoter {
    inner: bytes::Quoter,
}

impl Quoter {
    /// Create a new [`Quoter`] with default settings.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to allow [nul bytes](quoting_warning#nul-bytes).  By default they are not
    /// allowed and will result in an error of [`QuoteError::Nul`].
    #[inline]
    pub fn allow_nul(mut self, allow: bool) -> Self {
        self.inner = self.inner.allow_nul(allow);
        self
    }

    /// Convenience function that consumes an iterable of words and turns it into a single string,
    /// quoting words when necessary. Consecutive words will be separated by a single space.
    pub fn join<'a, I: IntoIterator<Item = &'a str>>(&self, words: I) -> Result<String, QuoteError> {
        // Safety: given valid UTF-8, bytes::join() will always return valid UTF-8.
        self.inner.join(words.into_iter().map(|s| s.as_bytes()))
            .map(|bytes| unsafe { String::from_utf8_unchecked(bytes) })
    }

    /// Given a single word, return a string suitable to encode it as a shell argument.
    pub fn quote<'a>(&self, in_str: &'a str) -> Result<Cow<'a, str>, QuoteError> {
        Ok(match self.inner.quote(in_str.as_bytes())? {
            Cow::Borrowed(out) => {
                // Safety: given valid UTF-8, bytes::quote() will always return valid UTF-8.
                unsafe { core::str::from_utf8_unchecked(out) }.into()
            }
            Cow::Owned(out) => {
                // Safety: given valid UTF-8, bytes::quote() will always return valid UTF-8.
                unsafe { String::from_utf8_unchecked(out) }.into()
            }
        })
    }
}

impl From<bytes::Quoter> for Quoter {
    fn from(inner: bytes::Quoter) -> Quoter {
        Quoter { inner }
    }
}

impl From<Quoter> for bytes::Quoter {
    fn from(quoter: Quoter) -> bytes::Quoter {
        quoter.inner
    }
}

/// Convenience function that consumes an iterable of words and turns it into a single string,
/// quoting words when necessary. Consecutive words will be separated by a single space.
///
/// Uses default settings except that nul bytes are passed through, which [may be
/// dangerous](quoting_warning#nul-bytes), leading to this function being deprecated.
///
/// Equivalent to [`Quoter::new().allow_nul(true).join(words).unwrap()`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The bytes equivalent is [bytes::join].
#[deprecated(since = "1.3.0", note = "replace with `try_join(words)?` to avoid nul byte danger")]
pub fn join<'a, I: IntoIterator<Item = &'a str>>(words: I) -> String {
    Quoter::new().allow_nul(true).join(words).unwrap()
}

/// Convenience function that consumes an iterable of words and turns it into a single string,
/// quoting words when necessary. Consecutive words will be separated by a single space.
///
/// Uses default settings.  The only error that can be returned is [`QuoteError::Nul`].
///
/// Equivalent to [`Quoter::new().join(words)`](Quoter).
///
/// The bytes equivalent is [bytes::try_join].
pub fn try_join<'a, I: IntoIterator<Item = &'a str>>(words: I) -> Result<String, QuoteError> {
    Quoter::new().join(words)
}

/// Given a single word, return a string suitable to encode it as a shell argument.
///
/// Uses default settings except that nul bytes are passed through, which [may be
/// dangerous](quoting_warning#nul-bytes), leading to this function being deprecated.
///
/// Equivalent to [`Quoter::new().allow_nul(true).quote(in_str).unwrap()`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The bytes equivalent is [bytes::quote].
#[deprecated(since = "1.3.0", note = "replace with `try_quote(str)?` to avoid nul byte danger")]
pub fn quote(in_str: &str) -> Cow<str> {
    Quoter::new().allow_nul(true).quote(in_str).unwrap()
}

/// Given a single word, return a string suitable to encode it as a shell argument.
///
/// Uses default settings.  The only error that can be returned is [`QuoteError::Nul`].
///
/// Equivalent to [`Quoter::new().quote(in_str)`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The bytes equivalent is [bytes::try_quote].
pub fn try_quote(in_str: &str) -> Result<Cow<str>, QuoteError> {
    Quoter::new().quote(in_str)
}

#[cfg(test)]
static SPLIT_TEST_ITEMS: &'static [(&'static str, Option<&'static [&'static str]>)] = &[
    ("foo$baz", Some(&["foo$baz"])),
    ("foo baz", Some(&["foo", "baz"])),
    ("foo\"bar\"baz", Some(&["foobarbaz"])),
    ("foo \"bar\"baz", Some(&["foo", "barbaz"])),
    ("   foo \nbar", Some(&["foo", "bar"])),
    ("foo\\\nbar", Some(&["foobar"])),
    ("\"foo\\\nbar\"", Some(&["foobar"])),
    ("'baz\\$b'", Some(&["baz\\$b"])),
    ("'baz\\\''", None),
    ("\\", None),
    ("\"\\", None),
    ("'\\", None),
    ("\"", None),
    ("'", None),
    ("foo #bar\nbaz", Some(&["foo", "baz"])),
    ("foo #bar", Some(&["foo"])),
    ("foo#bar", Some(&["foo#bar"])),
    ("foo\"#bar", None),
    ("'\\n'", Some(&["\\n"])),
    ("'\\\\n'", Some(&["\\\\n"])),
];

#[test]
fn test_split() {
    for &(input, output) in SPLIT_TEST_ITEMS {
        assert_eq!(split(input), output.map(|o| o.iter().map(|&x| x.to_owned()).collect()));
    }
}

#[test]
fn test_lineno() {
    let mut sh = Shlex::new("\nfoo\nbar");
    while let Some(word) = sh.next() {
        if word == "bar" {
            assert_eq!(sh.line_no, 3);
        }
    }
}

#[test]
#[cfg_attr(not(feature = "std"), allow(unreachable_code, unused_mut))]
fn test_quote() {
    // This is a list of (unquoted, quoted) pairs.
    // But it's using a single long (raw) string literal with an ad-hoc format, just because it's
    // hard to read if we have to put the test strings through Rust escaping on top of the escaping
    // being tested.  (Even raw string literals are noisy for short strings).
    // Ad-hoc: "NL" is replaced with a literal newline; no other escape sequences.
    let tests = r#"
        <>                => <''>
        <foobar>          => <foobar>
        <foo bar>         => <'foo bar'>
        <"foo bar'">      => <"\"foo bar'\"">
        <'foo bar'>       => <"'foo bar'">
        <">               => <'"'>
        <"'>              => <"\"'">
        <hello!world>     => <'hello!world'>
        <'hello!world>    => <"'hello"'!world'>
        <'hello!>         => <"'hello"'!'>
        <hello ^ world>   => <'hello ''^ world'>
        <hello^>          => <hello'^'>
        <!world'>         => <'!world'"'">
        <{a, b}>          => <'{a, b}'>
        <NL>              => <'NL'>
        <^>               => <'^'>
        <foo^bar>         => <foo'^bar'>
        <NLx^>            => <'NLx''^'>
        <NL^x>            => <'NL''^x'>
        <NL ^x>           => <'NL ''^x'>
        <{a,b}>           => <'{a,b}'>
        <a,b>             => <'a,b'>
        <a..b             => <a..b>
        <'$>              => <"'"'$'>
        <"^>              => <'"''^'>
    "#;
    let mut ok = true;
    for test in tests.trim().split('\n') {
        let parts: Vec<String> = test
            .replace("NL", "\n")
            .split("=>")
            .map(|part| part.trim().trim_start_matches('<').trim_end_matches('>').to_owned())
            .collect();
        assert!(parts.len() == 2);
        let unquoted = &*parts[0];
        let quoted_expected = &*parts[1];
        let quoted_actual = try_quote(&parts[0]).unwrap();
        if quoted_expected != quoted_actual {
            #[cfg(not(feature = "std"))]
            panic!("FAIL: for input <{}>, expected <{}>, got <{}>",
                     unquoted, quoted_expected, quoted_actual);
            #[cfg(feature = "std")]
            println!("FAIL: for input <{}>, expected <{}>, got <{}>",
                     unquoted, quoted_expected, quoted_actual);
            ok = false;
        }
    }
    assert!(ok);
}

#[test]
#[allow(deprecated)]
fn test_join() {
    assert_eq!(join(vec![]), "");
    assert_eq!(join(vec![""]), "''");
    assert_eq!(join(vec!["a", "b"]), "a b");
    assert_eq!(join(vec!["foo bar", "baz"]), "'foo bar' baz");
}

#[test]
fn test_fallible() {
    assert_eq!(try_join(vec!["\0"]), Err(QuoteError::Nul));
    assert_eq!(try_quote("\0"), Err(QuoteError::Nul));
}
