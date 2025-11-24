/*!
This crate provides a robust regular expression parser.

This crate defines two primary types:

* [`Ast`](ast/enum.Ast.html) is the abstract syntax of a regular expression.
  An abstract syntax corresponds to a *structured representation* of the
  concrete syntax of a regular expression, where the concrete syntax is the
  pattern string itself (e.g., `foo(bar)+`). Given some abstract syntax, it
  can be converted back to the original concrete syntax (modulo some details,
  like whitespace). To a first approximation, the abstract syntax is complex
  and difficult to analyze.
* [`Hir`](hir/struct.Hir.html) is the high-level intermediate representation
  ("HIR" or "high-level IR" for short) of regular expression. It corresponds to
  an intermediate state of a regular expression that sits between the abstract
  syntax and the low level compiled opcodes that are eventually responsible for
  executing a regular expression search. Given some high-level IR, it is not
  possible to produce the original concrete syntax (although it is possible to
  produce an equivalent concrete syntax, but it will likely scarcely resemble
  the original pattern). To a first approximation, the high-level IR is simple
  and easy to analyze.

These two types come with conversion routines:

* An [`ast::parse::Parser`](ast/parse/struct.Parser.html) converts concrete
  syntax (a `&str`) to an [`Ast`](ast/enum.Ast.html).
* A [`hir::translate::Translator`](hir/translate/struct.Translator.html)
  converts an [`Ast`](ast/enum.Ast.html) to a [`Hir`](hir/struct.Hir.html).

As a convenience, the above two conversion routines are combined into one via
the top-level [`Parser`](struct.Parser.html) type. This `Parser` will first
convert your pattern to an `Ast` and then convert the `Ast` to an `Hir`.


# Example

This example shows how to parse a pattern string into its HIR:

```
use regex_syntax::Parser;
use regex_syntax::hir::{self, Hir};

let hir = Parser::new().parse("a|b").unwrap();
assert_eq!(hir, Hir::alternation(vec![
    Hir::literal(hir::Literal::Unicode('a')),
    Hir::literal(hir::Literal::Unicode('b')),
]));
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

This crate provides limited support for
[literal extraction from `Hir` values](hir/literal/struct.Literals.html).
Be warned that literal extraction currently uses recursion, and therefore,
stack size proportional to the size of the `Hir`.

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
  [Uncode general categories](https://www.unicode.org/reports/tr44/tr44-24.html#General_Category_Values).
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
*/

#![deny(missing_docs)]
#![warn(missing_debug_implementations)]
#![forbid(unsafe_code)]

pub use crate::error::{Error, Result};
pub use crate::parser::{Parser, ParserBuilder};
pub use crate::unicode::UnicodeWordError;

pub mod ast;
mod either;
mod error;
pub mod hir;
mod parser;
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
/// These are the only characters that are allowed to be escaped, with one
/// exception: an ASCII space character may be escaped when extended mode (with
/// the `x` flag) is enabled. In particular, `is_meta_character(' ')` returns
/// `false`.
///
/// Note that the set of characters for which this function returns `true` or
/// `false` is fixed and won't change in a semver compatible release.
pub fn is_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{'
        | '}' | '^' | '$' | '#' | '&' | '-' | '~' => true,
        _ => false,
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
/// If the `unicode-perl` feature is not enabled, then this function panics.
/// For this reason, it is recommended that callers use
/// [`try_is_word_character`](fn.try_is_word_character.html)
/// instead.
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
) -> std::result::Result<bool, UnicodeWordError> {
    unicode::is_word_character(c)
}

/// Returns true if and only if the given character is an ASCII word character.
///
/// An ASCII word character is defined by the following character class:
/// `[_0-9a-zA-Z]'.
pub fn is_word_byte(c: u8) -> bool {
    match c {
        b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
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
