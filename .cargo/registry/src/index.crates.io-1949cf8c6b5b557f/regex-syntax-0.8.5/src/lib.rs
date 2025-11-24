/*!
This crate provides a robust regular expression parser.

This crate defines two primary types:

* [`Ast`](ast::Ast) is the abstract syntax of a regular expression.
  An abstract syntax corresponds to a *structured representation* of the
  concrete syntax of a regular expression, where the concrete syntax is the
  pattern string itself (e.g., `foo(bar)+`). Given some abstract syntax, it
  can be converted back to the original concrete syntax (modulo some details,
  like whitespace). To a first approximation, the abstract syntax is complex
  and difficult to analyze.
* [`Hir`](hir::Hir) is the high-level intermediate representation
  ("HIR" or "high-level IR" for short) of regular expression. It corresponds to
  an intermediate state of a regular expression that sits between the abstract
  syntax and the low level compiled opcodes that are eventually responsible for
  executing a regular expression search. Given some high-level IR, it is not
  possible to produce the original concrete syntax (although it is possible to
  produce an equivalent concrete syntax, but it will likely scarcely resemble
  the original pattern). To a first approximation, the high-level IR is simple
  and easy to analyze.

These two types come with conversion routines:

* An [`ast::parse::Parser`] converts concrete syntax (a `&str`) to an
[`Ast`](ast::Ast).
* A [`hir::translate::Translator`] converts an [`Ast`](ast::Ast) to a
[`Hir`](hir::Hir).

As a convenience, the above two conversion routines are combined into one via
the top-level [`Parser`] type. This `Parser` will first convert your pattern to
an `Ast` and then convert the `Ast` to an `Hir`. It's also exposed as top-level
[`parse`] free function.


# Example

This example shows how to parse a pattern string into its HIR:

```
use regex_syntax::{hir::Hir, parse};

let hir = parse("a|b")?;
assert_eq!(hir, Hir::alternation(vec![
    Hir::literal("a".as_bytes()),
    Hir::literal("b".as_bytes()),
]));
# Ok::<(), Box<dyn std::error::Error>>(())
```


# Concrete syntax supported

The concrete syntax is documented as part of the public API of the
[`regex` crate](https://docs.rs/regex/%2A/regex/#syntax).


# Input safety

A key feature of this library is that it is safe to use with end user facing
input. This plays a significant role in the internal implementation. In
particular:

1. Parsers provide a `nest_limit` option that permits callers to control how
   deeply nested a regular expression is allowed to be. This makes it possible
   to do case analysis over an `Ast` or an `Hir` using recursion without
   worrying about stack overflow.
2. Since relying on a particular stack size is brittle, this crate goes to
   great lengths to ensure that all interactions with both the `Ast` and the
   `Hir` do not use recursion. Namely, they use constant stack space and heap
   space proportional to the size of the original pattern string (in bytes).
   This includes the type's corresponding destructors. (One exception to this
   is literal extraction, but this will eventually get fixed.)


# Error reporting

The `Display` implementations on all `Error` types exposed in this library
provide nice human readable errors that are suitable for showing to end users
in a monospace font.


# Literal extraction

This crate provides limited support for [literal extraction from `Hir`
values](hir::literal). Be warned that literal extraction uses recursion, and
therefore, stack size proportional to the size of the `Hir`.

The purpose of literal extraction is to speed up searches. That is, if you
know a regular expression must match a prefix or suffix literal, then it is
often quicker to search for instances of that literal, and then confirm or deny
the match using the full regular expression engine. These optimizations are
done automatically in the `regex` crate.


# Crate features

An important feature provided by this crate is its Unicode support. This
includes things like case folding, boolean properties, general categories,
scripts and Unicode-aware support for the Perl classes `\w`, `\s` and `\d`.
However, a downside of this support is that it requires bundling several
Unicode data tables that are substantial in size.

A fair number of use cases do not require full Unicode support. For this
reason, this crate exposes a number of features to control which Unicode
data is available.

If a regular expression attempts to use a Unicode feature that is not available
because the corresponding crate feature was disabled, then translating that
regular expression to an `Hir` will return an error. (It is still possible
construct an `Ast` for such a regular expression, since Unicode data is not
used until translation to an `Hir`.) Stated differently, enabling or disabling
any of the features below can only add or subtract from the total set of valid
regular expressions. Enabling or disabling a feature will never modify the
match semantics of a regular expression.

The following features are available:

* **std** -
  Enables support for the standard library. This feature is enabled by default.
  When disabled, only `core` and `alloc` are used. Otherwise, enabling `std`
  generally just enables `std::error::Error` trait impls for the various error
  types.
* **unicode** -
  Enables all Unicode features. This feature is enabled by default, and will
  always cover all Unicode features, even if more are added in the future.
* **unicode-age** -
  Provide the data for the
  [Unicode `Age` property](https://www.unicode.org/reports/tr44/tr44-24.html#Character_Age).
  This makes it possible to use classes like `\p{Age:6.0}` to refer to all
  codepoints first introduced in Unicode 6.0
* **unicode-bool** -
  Provide the data for numerous Unicode boolean properties. The full list
  is not included here, but contains properties like `Alphabetic`, `Emoji`,
  `Lowercase`, `Math`, `Uppercase` and `White_Space`.
* **unicode-case** -
  Provide the data for case insensitive matching using
  [Unicode's "simple loose matches" specification](https://www.unicode.org/reports/tr18/#Simple_Loose_Matches).
* **unicode-gencat** -
  Provide the data for
  [Unicode general categories](https://www.unicode.org/reports/tr44/tr44-24.html#General_Category_Values).
  This includes, but is not limited to, `Decimal_Number`, `Letter`,
  `Math_Symbol`, `Number` and `Punctuation`.
* **unicode-perl** -
  Provide the data for supporting the Unicode-aware Perl character classes,
  corresponding to `\w`, `\s` and `\d`. This is also necessary for using
  Unicode-aware word boundary assertions. Note that if this feature is
  disabled, the `\s` and `\d` character classes are still available if the
  `unicode-bool` and `unicode-gencat` features are enabled, respectively.
* **unicode-script** -
  Provide the data for
  [Unicode scripts and script extensions](https://www.unicode.org/reports/tr24/).
  This includes, but is not limited to, `Arabic`, `Cyrillic`, `Hebrew`,
  `Latin` and `Thai`.
* **unicode-segment** -
  Provide the data necessary to provide the properties used to implement the
  [Unicode text segmentation algorithms](https://www.unicode.org/reports/tr29/).
  This enables using classes like `\p{gcb=Extend}`, `\p{wb=Katakana}` and
  `\p{sb=ATerm}`.
* **arbitrary** -
  Enabling this feature introduces a public dependency on the
  [`arbitrary`](https://crates.io/crates/arbitrary)
  crate. Namely, it implements the `Arbitrary` trait from that crate for the
  [`Ast`](crate::ast::Ast) type. This feature is disabled by default.
*/

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![warn(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(any(test, feature = "std"))]
extern crate std;

extern crate alloc;

pub use crate::{
    error::Error,
    parser::{parse, Parser, ParserBuilder},
    unicode::UnicodeWordError,
};

use alloc::string::String;

pub mod ast;
mod debug;
mod either;
mod error;
pub mod hir;
mod parser;
mod rank;
mod unicode;
mod unicode_tables;
pub mod utf8;

/// Escapes all regular expression meta characters in `text`.
///
/// The string returned may be safely used as a literal in a regular
/// expression.
pub fn escape(text: &str) -> String {
    let mut quoted = String::new();
    escape_into(text, &mut quoted);
    quoted
}

/// Escapes all meta characters in `text` and writes the result into `buf`.
///
/// This will append escape characters into the given buffer. The characters
/// that are appended are safe to use as a literal in a regular expression.
pub fn escape_into(text: &str, buf: &mut String) {
    buf.reserve(text.len());
    for c in text.chars() {
        if is_meta_character(c) {
            buf.push('\\');
        }
        buf.push(c);
    }
}

/// Returns true if the given character has significance in a regex.
///
/// Generally speaking, these are the only characters which _must_ be escaped
/// in order to match their literal meaning. For example, to match a literal
/// `|`, one could write `\|`. Sometimes escaping isn't always necessary. For
/// example, `-` is treated as a meta character because of its significance
/// for writing ranges inside of character classes, but the regex `-` will
/// match a literal `-` because `-` has no special meaning outside of character
/// classes.
///
/// In order to determine whether a character may be escaped at all, the
/// [`is_escapeable_character`] routine should be used. The difference between
/// `is_meta_character` and `is_escapeable_character` is that the latter will
/// return true for some characters that are _not_ meta characters. For
/// example, `%` and `\%` both match a literal `%` in all contexts. In other
/// words, `is_escapeable_character` includes "superfluous" escapes.
///
/// Note that the set of characters for which this function returns `true` or
/// `false` is fixed and won't change in a semver compatible release. (In this
/// case, "semver compatible release" actually refers to the `regex` crate
/// itself, since reducing or expanding the set of meta characters would be a
/// breaking change for not just `regex-syntax` but also `regex` itself.)
///
/// # Example
///
/// ```
/// use regex_syntax::is_meta_character;
///
/// assert!(is_meta_character('?'));
/// assert!(is_meta_character('-'));
/// assert!(is_meta_character('&'));
/// assert!(is_meta_character('#'));
///
/// assert!(!is_meta_character('%'));
/// assert!(!is_meta_character('/'));
/// assert!(!is_meta_character('!'));
/// assert!(!is_meta_character('"'));
/// assert!(!is_meta_character('e'));
/// ```
pub fn is_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{'
        | '}' | '^' | '$' | '#' | '&' | '-' | '~' => true,
        _ => false,
    }
}

/// Returns true if the given character can be escaped in a regex.
///
/// This returns true in all cases that `is_meta_character` returns true, but
/// also returns true in some cases where `is_meta_character` returns false.
/// For example, `%` is not a meta character, but it is escapeable. That is,
/// `%` and `\%` both match a literal `%` in all contexts.
///
/// The purpose of this routine is to provide knowledge about what characters
/// may be escaped. Namely, most regex engines permit "superfluous" escapes
/// where characters without any special significance may be escaped even
/// though there is no actual _need_ to do so.
///
/// This will return false for some characters. For example, `e` is not
/// escapeable. Therefore, `\e` will either result in a parse error (which is
/// true today), or it could backwards compatibly evolve into a new construct
/// with its own meaning. Indeed, that is the purpose of banning _some_
/// superfluous escapes: it provides a way to evolve the syntax in a compatible
/// manner.
///
/// # Example
///
/// ```
/// use regex_syntax::is_escapeable_character;
///
/// assert!(is_escapeable_character('?'));
/// assert!(is_escapeable_character('-'));
/// assert!(is_escapeable_character('&'));
/// assert!(is_escapeable_character('#'));
/// assert!(is_escapeable_character('%'));
/// assert!(is_escapeable_character('/'));
/// assert!(is_escapeable_character('!'));
/// assert!(is_escapeable_character('"'));
///
/// assert!(!is_escapeable_character('e'));
/// ```
pub fn is_escapeable_character(c: char) -> bool {
    // Certainly escapeable if it's a meta character.
    if is_meta_character(c) {
        return true;
    }
    // Any character that isn't ASCII is definitely not escapeable. There's
    // no real need to allow things like \☃ right?
    if !c.is_ascii() {
        return false;
    }
    // Otherwise, we basically say that everything is escapeable unless it's a
    // letter or digit. Things like \3 are either octal (when enabled) or an
    // error, and we should keep it that way. Otherwise, letters are reserved
    // for adding new syntax in a backwards compatible way.
    match c {
        '0'..='9' | 'A'..='Z' | 'a'..='z' => false,
        // While not currently supported, we keep these as not escapeable to
        // give us some flexibility with respect to supporting the \< and
        // \> word boundary assertions in the future. By rejecting them as
        // escapeable, \< and \> will result in a parse error. Thus, we can
        // turn them into something else in the future without it being a
        // backwards incompatible change.
        //
        // OK, now we support \< and \>, and we need to retain them as *not*
        // escapeable here since the escape sequence is significant.
        '<' | '>' => false,
        _ => true,
    }
}

/// Returns true if and only if the given character is a Unicode word
/// character.
///
/// A Unicode word character is defined by
/// [UTS#18 Annex C](https://unicode.org/reports/tr18/#Compatibility_Properties).
/// In particular, a character
/// is considered a word character if it is in either of the `Alphabetic` or
/// `Join_Control` properties, or is in one of the `Decimal_Number`, `Mark`
/// or `Connector_Punctuation` general categories.
///
/// # Panics
///
/// If the `unicode-perl` feature is not enabled, then this function
/// panics. For this reason, it is recommended that callers use
/// [`try_is_word_character`] instead.
pub fn is_word_character(c: char) -> bool {
    try_is_word_character(c).expect("unicode-perl feature must be enabled")
}

/// Returns true if and only if the given character is a Unicode word
/// character.
///
/// A Unicode word character is defined by
/// [UTS#18 Annex C](https://unicode.org/reports/tr18/#Compatibility_Properties).
/// In particular, a character
/// is considered a word character if it is in either of the `Alphabetic` or
/// `Join_Control` properties, or is in one of the `Decimal_Number`, `Mark`
/// or `Connector_Punctuation` general categories.
///
/// # Errors
///
/// If the `unicode-perl` feature is not enabled, then this function always
/// returns an error.
pub fn try_is_word_character(
    c: char,
) -> core::result::Result<bool, UnicodeWordError> {
    unicode::is_word_character(c)
}

/// Returns true if and only if the given character is an ASCII word character.
///
/// An ASCII word character is defined by the following character class:
/// `[_0-9a-zA-Z]`.
pub fn is_word_byte(c: u8) -> bool {
    match c {
        b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn escape_meta() {
        assert_eq!(
            escape(r"\.+*?()|[]{}^$#&-~"),
            r"\\\.\+\*\?\(\)\|\[\]\{\}\^\$\#\&\-\~".to_string()
        );
    }

    #[test]
    fn word_byte() {
        assert!(is_word_byte(b'a'));
        assert!(!is_word_byte(b'-'));
    }

    #[test]
    #[cfg(feature = "unicode-perl")]
    fn word_char() {
        assert!(is_word_character('a'), "ASCII");
        assert!(is_word_character('à'), "Latin-1");
        assert!(is_word_character('β'), "Greek");
        assert!(is_word_character('\u{11011}'), "Brahmi (Unicode 6.0)");
        assert!(is_word_character('\u{11611}'), "Modi (Unicode 7.0)");
        assert!(is_word_character('\u{11711}'), "Ahom (Unicode 8.0)");
        assert!(is_word_character('\u{17828}'), "Tangut (Unicode 9.0)");
        assert!(is_word_character('\u{1B1B1}'), "Nushu (Unicode 10.0)");
        assert!(is_word_character('\u{16E40}'), "Medefaidrin (Unicode 11.0)");
        assert!(!is_word_character('-'));
        assert!(!is_word_character('☃'));
    }

    #[test]
    #[should_panic]
    #[cfg(not(feature = "unicode-perl"))]
    fn word_char_disabled_panic() {
        assert!(is_word_character('a'));
    }

    #[test]
    #[cfg(not(feature = "unicode-perl"))]
    fn word_char_disabled_error() {
        assert!(try_is_word_character('a').is_err());
    }
}
