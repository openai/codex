// Copyright 2015 Nicholas Allegra (comex).
// Licensed under the Apache License, Version 2.0 <https://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license <https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! [`Shlex`] and friends for byte strings.
//!
//! This is used internally by the [outer module](crate), and may be more
//! convenient if you are working with byte slices (`[u8]`) or types that are
//! wrappers around bytes, such as [`OsStr`](std::ffi::OsStr):
//!
//! ```rust
//! #[cfg(unix)] {
//!     use shlex::bytes::quote;
//!     use std::ffi::OsStr;
//!     use std::os::unix::ffi::OsStrExt;
//!
//!     // `\x80` is invalid in UTF-8.
//!     let os_str = OsStr::from_bytes(b"a\x80b c");
//!     assert_eq!(quote(os_str.as_bytes()), &b"'a\x80b c'"[..]);
//! }
//! ```
//!
//! (On Windows, `OsStr` uses 16 bit wide characters so this will not work.)

extern crate alloc;
use alloc::vec::Vec;
use alloc::borrow::Cow;
#[cfg(test)]
use alloc::vec;
#[cfg(test)]
use alloc::borrow::ToOwned;
#[cfg(all(doc, not(doctest)))]
use crate::{self as shlex, quoting_warning};

use super::QuoteError;

/// An iterator that takes an input byte string and splits it into the words using the same syntax as
/// the POSIX shell.
pub struct Shlex<'a> {
    in_iter: core::slice::Iter<'a, u8>,
    /// The number of newlines read so far, plus one.
    pub line_no: usize,
    /// An input string is erroneous if it ends while inside a quotation or right after an
    /// unescaped backslash.  Since Iterator does not have a mechanism to return an error, if that
    /// happens, Shlex just throws out the last token, ends the iteration, and sets 'had_error' to
    /// true; best to check it after you're done iterating.
    pub had_error: bool,
}

impl<'a> Shlex<'a> {
    pub fn new(in_bytes: &'a [u8]) -> Self {
        Shlex {
            in_iter: in_bytes.iter(),
            line_no: 1,
            had_error: false,
        }
    }

    fn parse_word(&mut self, mut ch: u8) -> Option<Vec<u8>> {
        let mut result: Vec<u8> = Vec::new();
        loop {
            match ch as char {
                '"' => if let Err(()) = self.parse_double(&mut result) {
                    self.had_error = true;
                    return None;
                },
                '\'' => if let Err(()) = self.parse_single(&mut result) {
                    self.had_error = true;
                    return None;
                },
                '\\' => if let Some(ch2) = self.next_char() {
                    if ch2 != '\n' as u8 { result.push(ch2); }
                } else {
                    self.had_error = true;
                    return None;
                },
                ' ' | '\t' | '\n' => { break; },
                _ => { result.push(ch as u8); },
            }
            if let Some(ch2) = self.next_char() { ch = ch2; } else { break; }
        }
        Some(result)
    }

    fn parse_double(&mut self, result: &mut Vec<u8>) -> Result<(), ()> {
        loop {
            if let Some(ch2) = self.next_char() {
                match ch2 as char {
                    '\\' => {
                        if let Some(ch3) = self.next_char() {
                            match ch3 as char {
                                // \$ => $
                                '$' | '`' | '"' | '\\' => { result.push(ch3); },
                                // \<newline> => nothing
                                '\n' => {},
                                // \x => =x
                                _ => { result.push('\\' as u8); result.push(ch3); }
                            }
                        } else {
                            return Err(());
                        }
                    },
                    '"' => { return Ok(()); },
                    _ => { result.push(ch2); },
                }
            } else {
                return Err(());
            }
        }
    }

    fn parse_single(&mut self, result: &mut Vec<u8>) -> Result<(), ()> {
        loop {
            if let Some(ch2) = self.next_char() {
                match ch2 as char {
                    '\'' => { return Ok(()); },
                    _ => { result.push(ch2); },
                }
            } else {
                return Err(());
            }
        }
    }

    fn next_char(&mut self) -> Option<u8> {
        let res = self.in_iter.next().copied();
        if res == Some(b'\n') { self.line_no += 1; }
        res
    }
}

impl<'a> Iterator for Shlex<'a> {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut ch) = self.next_char() {
            // skip initial whitespace
            loop {
                match ch as char {
                    ' ' | '\t' | '\n' => {},
                    '#' => {
                        while let Some(ch2) = self.next_char() {
                            if ch2 as char == '\n' { break; }
                        }
                    },
                    _ => { break; }
                }
                if let Some(ch2) = self.next_char() { ch = ch2; } else { return None; }
            }
            self.parse_word(ch)
        } else { // no initial character
            None
        }
    }

}

/// Convenience function that consumes the whole byte string at once.  Returns None if the input was
/// erroneous.
pub fn split(in_bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut shl = Shlex::new(in_bytes);
    let res = shl.by_ref().collect();
    if shl.had_error { None } else { Some(res) }
}

/// A more configurable interface to quote strings.  If you only want the default settings you can
/// use the convenience functions [`try_quote`] and [`try_join`].
///
/// The string equivalent is [`shlex::Quoter`].
#[derive(Default, Debug, Clone)]
pub struct Quoter {
    allow_nul: bool,
    // TODO: more options
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
        self.allow_nul = allow;
        self
    }

    /// Convenience function that consumes an iterable of words and turns it into a single byte string,
    /// quoting words when necessary. Consecutive words will be separated by a single space.
    pub fn join<'a, I: IntoIterator<Item = &'a [u8]>>(&self, words: I) -> Result<Vec<u8>, QuoteError> {
        Ok(words.into_iter()
            .map(|word| self.quote(word))
            .collect::<Result<Vec<Cow<[u8]>>, QuoteError>>()?
            .join(&b' '))
    }

    /// Given a single word, return a byte string suitable to encode it as a shell argument.
    ///
    /// If given valid UTF-8, this will never produce invalid UTF-8. This is because it only
    /// ever inserts valid ASCII characters before or after existing ASCII characters (or
    /// returns two single quotes if the input was an empty string). It will never modify a
    /// multibyte UTF-8 character.
    pub fn quote<'a>(&self, mut in_bytes: &'a [u8]) -> Result<Cow<'a, [u8]>, QuoteError> {
        if in_bytes.is_empty() {
            // Empty string.  Special case that isn't meaningful as only part of a word.
            return Ok(b"''"[..].into());
        }
        if !self.allow_nul && in_bytes.iter().any(|&b| b == b'\0') {
            return Err(QuoteError::Nul);
        }
        let mut out: Vec<u8> = Vec::new();
        while !in_bytes.is_empty() {
            // Pick a quoting strategy for some prefix of the input.  Normally this will cover the
            // entire input, but in some case we might need to divide the input into multiple chunks
            // that are quoted differently.
            let (cur_len, strategy) = quoting_strategy(in_bytes);
            if cur_len == in_bytes.len() && strategy == QuotingStrategy::Unquoted && out.is_empty() {
                // Entire string can be represented unquoted.  Reuse the allocation.
                return Ok(in_bytes.into());
            }
            let (cur_chunk, rest) = in_bytes.split_at(cur_len);
            assert!(rest.len() < in_bytes.len()); // no infinite loop
            in_bytes = rest;
            append_quoted_chunk(&mut out, cur_chunk, strategy);
        }
        Ok(out.into())
    }

}

#[derive(PartialEq)]
enum QuotingStrategy {
    /// No quotes and no backslash escapes.  (If backslash escapes would be necessary, we use a
    /// different strategy instead.)
    Unquoted,
    /// Single quoted.
    SingleQuoted,
    /// Double quotes, potentially with backslash escapes.
    DoubleQuoted,
    // TODO: add $'xxx' and "$(printf 'xxx')" styles
}

/// Is this ASCII byte okay to emit unquoted?
const fn unquoted_ok(c: u8) -> bool {
    match c as char {
        // Allowed characters:
        '+' | '-' | '.' | '/' | ':' | '@' | ']' | '_' |
        '0'..='9' | 'A'..='Z' | 'a'..='z'
        => true,

        // Non-allowed characters:
        // From POSIX https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html
        // "The application shall quote the following characters if they are to represent themselves:"
        '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '"' | '\'' | ' ' | '\t' | '\n' |
        // "and the following may need to be quoted under certain circumstances[..]:"
        '*' | '?' | '[' | '#' | '~' | '=' | '%' |
        // Brace expansion.  These ought to be in the POSIX list but aren't yet;
        // see: https://www.austingroupbugs.net/view.php?id=1193
        '{' | '}' |
        // Also quote comma, just to be safe in the extremely odd case that the user of this crate
        // is intentionally placing a quoted string inside a brace expansion, e.g.:
        //     format!("echo foo{{a,b,{}}}" | shlex::quote(some_str))
        ',' |
        // '\r' is allowed in a word by all real shells I tested, but is treated as a word
        // separator by Python `shlex` | and might be translated to '\n' in interactive mode.
        '\r' |
        // '!' and '^' are treated specially in interactive mode; see quoting_warning.
        '!' | '^' |
        // Nul bytes and control characters.
        '\x00' ..= '\x1f' | '\x7f'
        => false,
        '\u{80}' ..= '\u{10ffff}' => {
            // This is unreachable since `unquoted_ok` is only called for 0..128.
            // Non-ASCII bytes are handled separately in `quoting_strategy`.
            // Can't call unreachable!() from `const fn` on old Rust, so...
            unquoted_ok(c)
        },
    }
    // Note: The logic cited above for quoting comma might suggest that `..` should also be quoted,
    // it as a special case of brace expansion).  But it's not necessary.  There are three cases:
    //
    // 1. The user wants comma-based brace expansion, but the untrusted string being `quote`d
    //    contains `..`, so they get something like `{foo,bar,3..5}`.
    //  => That's safe; both Bash and Zsh expand this to `foo bar 3..5` rather than
    //     `foo bar 3 4 5`.  The presence of commas disables sequence expression expansion.
    //
    // 2. The user wants comma-based brace expansion where the contents of the braces are a
    //    variable number of `quote`d strings and nothing else.  There happens to be exactly
    //    one string and it contains `..`, so they get something like `{3..5}`.
    //  => Then this will expand as a sequence expression, which is unintended.  But I don't mind,
    //     because any such code is already buggy.  Suppose the untrusted string *didn't* contain
    //     `,` or `..`, resulting in shell input like `{foo}`.  Then the shell would interpret it
    //     as the literal string `{foo}` rather than brace-expanding it into `foo`.
    //
    // 3. The user wants a sequence expression and wants to supply an untrusted string as one of
    //    the endpoints or the increment.
    //  => Well, that's just silly, since the endpoints can only be numbers or single letters.
}

/// Optimized version of `unquoted_ok`.
fn unquoted_ok_fast(c: u8) -> bool {
    const UNQUOTED_OK_MASK: u128 = {
        // Make a mask of all bytes in 0..<0x80 that pass.
        let mut c = 0u8;
        let mut mask = 0u128;
        while c < 0x80 {
            if unquoted_ok(c) {
                mask |= 1u128 << c;
            }
            c += 1;
        }
        mask
    };
    ((UNQUOTED_OK_MASK >> c) & 1) != 0
}

/// Is this ASCII byte okay to emit in single quotes?
fn single_quoted_ok(c: u8) -> bool {
    match c {
        // No single quotes in single quotes.
        b'\'' => false,
        // To work around a Bash bug, ^ is only allowed right after an opening single quote; see
        // quoting_warning.
        b'^' => false,
        // Backslashes in single quotes are literal according to POSIX, but Fish treats them as an
        // escape character.  Ban them.  Fish doesn't aim to be POSIX-compatible, but we *can*
        // achieve Fish compatibility using double quotes, so we might as well.
        b'\\' => false,
        _ => true
    }
}

/// Is this ASCII byte okay to emit in double quotes?
fn double_quoted_ok(c: u8) -> bool {
    match c {
        // Work around Python `shlex` bug where parsing "\`" and "\$" doesn't strip the
        // backslash, even though POSIX requires it.
        b'`' | b'$' => false,
        // '!' and '^' are treated specially in interactive mode; see quoting_warning.
        b'!' | b'^' => false,
        _ => true
    }
}

/// Given an input, return a quoting strategy that can cover some prefix of the string, along with
/// the size of that prefix.
///
/// Precondition: input size is nonzero.  (Empty strings are handled by the caller.)
/// Postcondition: returned size is nonzero.
#[cfg_attr(manual_codegen_check, inline(never))]
fn quoting_strategy(in_bytes: &[u8]) -> (usize, QuotingStrategy) {
    const UNQUOTED_OK: u8 = 1;
    const SINGLE_QUOTED_OK: u8 = 2;
    const DOUBLE_QUOTED_OK: u8 = 4;

    let mut prev_ok = SINGLE_QUOTED_OK | DOUBLE_QUOTED_OK | UNQUOTED_OK;
    let mut i = 0;

    if in_bytes[0] == b'^' {
        // To work around a Bash bug, ^ is only allowed right after an opening single quote; see
        // quoting_warning.
        prev_ok = SINGLE_QUOTED_OK;
        i = 1;
    }

    while i < in_bytes.len() {
        let c = in_bytes[i];
        let mut cur_ok = prev_ok;

        if c >= 0x80 {
            // Normally, non-ASCII characters shouldn't require quoting, but see quoting_warning.md
            // about \xa0.  For now, just treat all non-ASCII characters as requiring quotes.  This
            // also ensures things are safe in the off-chance that you're in a legacy 8-bit locale that
            // has additional characters satisfying `isblank`.
            cur_ok &= !UNQUOTED_OK;
        } else {
            if !unquoted_ok_fast(c) {
                cur_ok &= !UNQUOTED_OK;
            }
            if !single_quoted_ok(c){
                cur_ok &= !SINGLE_QUOTED_OK;
            }
            if !double_quoted_ok(c) {
                cur_ok &= !DOUBLE_QUOTED_OK;
            }
        }

        if cur_ok == 0 {
            // There are no quoting strategies that would work for both the previous characters and
            // this one.  So we have to end the chunk before this character.  The caller will call
            // `quoting_strategy` again to handle the rest of the string.
            break;
        }

        prev_ok = cur_ok;
        i += 1;
    }

    // Pick the best allowed strategy.
    let strategy = if prev_ok & UNQUOTED_OK != 0 {
        QuotingStrategy::Unquoted
    } else if prev_ok & SINGLE_QUOTED_OK != 0 {
        QuotingStrategy::SingleQuoted
    } else if prev_ok & DOUBLE_QUOTED_OK != 0 {
        QuotingStrategy::DoubleQuoted
    } else {
        unreachable!()
    };
    debug_assert!(i > 0);
    (i, strategy)
}

fn append_quoted_chunk(out: &mut Vec<u8>, cur_chunk: &[u8], strategy: QuotingStrategy) {
    match strategy {
        QuotingStrategy::Unquoted => {
            out.extend_from_slice(cur_chunk);
        },
        QuotingStrategy::SingleQuoted => {
            out.reserve(cur_chunk.len() + 2);
            out.push(b'\'');
            out.extend_from_slice(cur_chunk);
            out.push(b'\'');
        },
        QuotingStrategy::DoubleQuoted => {
            out.reserve(cur_chunk.len() + 2);
            out.push(b'"');
            for &c in cur_chunk.into_iter() {
                if let b'$' | b'`' | b'"' | b'\\' = c {
                    // Add a preceding backslash.
                    // Note: We shouldn't actually get here for $ and ` because they don't pass
                    // `double_quoted_ok`.
                    out.push(b'\\');
                }
                // Add the character itself.
                out.push(c);
            }
            out.push(b'"');
        },
    }
}

/// Convenience function that consumes an iterable of words and turns it into a single byte string,
/// quoting words when necessary. Consecutive words will be separated by a single space.
///
/// Uses default settings except that nul bytes are passed through, which [may be
/// dangerous](quoting_warning#nul-bytes), leading to this function being deprecated.
///
/// Equivalent to [`Quoter::new().allow_nul(true).join(words).unwrap()`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The string equivalent is [shlex::join].
#[deprecated(since = "1.3.0", note = "replace with `try_join(words)?` to avoid nul byte danger")]
pub fn join<'a, I: IntoIterator<Item = &'a [u8]>>(words: I) -> Vec<u8> {
    Quoter::new().allow_nul(true).join(words).unwrap()
}

/// Convenience function that consumes an iterable of words and turns it into a single byte string,
/// quoting words when necessary. Consecutive words will be separated by a single space.
///
/// Uses default settings.  The only error that can be returned is [`QuoteError::Nul`].
///
/// Equivalent to [`Quoter::new().join(words)`](Quoter).
///
/// The string equivalent is [shlex::try_join].
pub fn try_join<'a, I: IntoIterator<Item = &'a [u8]>>(words: I) -> Result<Vec<u8>, QuoteError> {
    Quoter::new().join(words)
}

/// Given a single word, return a string suitable to encode it as a shell argument.
///
/// Uses default settings except that nul bytes are passed through, which [may be
/// dangerous](quoting_warning#nul-bytes), leading to this function being deprecated.
///
/// Equivalent to [`Quoter::new().allow_nul(true).quote(in_bytes).unwrap()`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The string equivalent is [shlex::quote].
#[deprecated(since = "1.3.0", note = "replace with `try_quote(str)?` to avoid nul byte danger")]
pub fn quote(in_bytes: &[u8]) -> Cow<[u8]> {
    Quoter::new().allow_nul(true).quote(in_bytes).unwrap()
}

/// Given a single word, return a string suitable to encode it as a shell argument.
///
/// Uses default settings.  The only error that can be returned is [`QuoteError::Nul`].
///
/// Equivalent to [`Quoter::new().quote(in_bytes)`](Quoter).
///
/// (That configuration never returns `Err`, so this function does not panic.)
///
/// The string equivalent is [shlex::try_quote].
pub fn try_quote(in_bytes: &[u8]) -> Result<Cow<[u8]>, QuoteError> {
    Quoter::new().quote(in_bytes)
}

#[cfg(test)]
const INVALID_UTF8: &[u8] = b"\xa1";
#[cfg(test)]
const INVALID_UTF8_SINGLEQUOTED: &[u8] = b"'\xa1'";

#[test]
#[allow(invalid_from_utf8)]
fn test_invalid_utf8() {
    // Check that our test string is actually invalid UTF-8.
    assert!(core::str::from_utf8(INVALID_UTF8).is_err());
}

#[cfg(test)]
static SPLIT_TEST_ITEMS: &'static [(&'static [u8], Option<&'static [&'static [u8]]>)] = &[
    (b"foo$baz", Some(&[b"foo$baz"])),
    (b"foo baz", Some(&[b"foo", b"baz"])),
    (b"foo\"bar\"baz", Some(&[b"foobarbaz"])),
    (b"foo \"bar\"baz", Some(&[b"foo", b"barbaz"])),
    (b"   foo \nbar", Some(&[b"foo", b"bar"])),
    (b"foo\\\nbar", Some(&[b"foobar"])),
    (b"\"foo\\\nbar\"", Some(&[b"foobar"])),
    (b"'baz\\$b'", Some(&[b"baz\\$b"])),
    (b"'baz\\\''", None),
    (b"\\", None),
    (b"\"\\", None),
    (b"'\\", None),
    (b"\"", None),
    (b"'", None),
    (b"foo #bar\nbaz", Some(&[b"foo", b"baz"])),
    (b"foo #bar", Some(&[b"foo"])),
    (b"foo#bar", Some(&[b"foo#bar"])),
    (b"foo\"#bar", None),
    (b"'\\n'", Some(&[b"\\n"])),
    (b"'\\\\n'", Some(&[b"\\\\n"])),
    (INVALID_UTF8, Some(&[INVALID_UTF8])),
];

#[test]
fn test_split() {
    for &(input, output) in SPLIT_TEST_ITEMS {
        assert_eq!(split(input), output.map(|o| o.iter().map(|&x| x.to_owned()).collect()));
    }
}

#[test]
fn test_lineno() {
    let mut sh = Shlex::new(b"\nfoo\nbar");
    while let Some(word) = sh.next() {
        if word == b"bar" {
            assert_eq!(sh.line_no, 3);
        }
    }
}

#[test]
#[allow(deprecated)]
fn test_quote() {
    // Validate behavior with invalid UTF-8:
    assert_eq!(quote(INVALID_UTF8), INVALID_UTF8_SINGLEQUOTED);
    // Replicate a few tests from lib.rs.  No need to replicate all of them.
    assert_eq!(quote(b""), &b"''"[..]);
    assert_eq!(quote(b"foobar"), &b"foobar"[..]);
    assert_eq!(quote(b"foo bar"), &b"'foo bar'"[..]);
    assert_eq!(quote(b"'\""), &b"\"'\\\"\""[..]);
    assert_eq!(quote(b""), &b"''"[..]);
}

#[test]
#[allow(deprecated)]
fn test_join() {
    // Validate behavior with invalid UTF-8:
    assert_eq!(join(vec![INVALID_UTF8]), INVALID_UTF8_SINGLEQUOTED);
    // Replicate a few tests from lib.rs.  No need to replicate all of them.
    assert_eq!(join(vec![]), &b""[..]);
    assert_eq!(join(vec![&b""[..]]), b"''");
}
