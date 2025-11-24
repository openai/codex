//! This example shows an example of how to parse an escaped string. The
//! rules for the string are similar to JSON and rust. A string is:
//!
//! - Enclosed by double quotes
//! - Can contain any raw unescaped code point besides \ and "
//! - Matches the following escape sequences: \b, \f, \n, \r, \t, \", \\, \/
//! - Matches code points like Rust: \u{XXXX}, where XXXX can be up to 6
//!   hex characters
//! - an escape followed by whitespace consumes all whitespace between the
//!   escape and the next non-whitespace character

use winnow::ascii::multispace1;
use winnow::combinator::alt;
use winnow::combinator::repeat;
use winnow::combinator::{delimited, preceded};
use winnow::error::{FromExternalError, ParserError};
use winnow::prelude::*;
use winnow::token::{take_till, take_while};
use winnow::Result;

/// Parse a string. Use a loop of `parse_fragment` and push all of the fragments
/// into an output string.
pub(crate) fn parse_string<'a, E>(input: &mut &'a str) -> Result<String, E>
where
    E: ParserError<&'a str> + FromExternalError<&'a str, std::num::ParseIntError>,
{
    // Repeat::fold is the equivalent of iterator::fold. It runs a parser in a loop,
    // and for each output value, calls a folding function on each output value.
    let build_string = repeat(
        0..,
        // Our parser function – parses a single string fragment
        parse_fragment,
    )
    .fold(
        // Our init value, an empty string
        String::new,
        // Our folding function. For each fragment, append the fragment to the
        // string.
        |mut string, fragment| {
            match fragment {
                StringFragment::Literal(s) => string.push_str(s),
                StringFragment::EscapedChar(c) => string.push(c),
                StringFragment::EscapedWS => {}
            }
            string
        },
    );

    // Finally, parse the string. Note that, if `build_string` could accept a raw
    // " character, the closing delimiter " would never match. When using
    // `delimited` with a looping parser (like Repeat::fold), be sure that the
    // loop won't accidentally match your closing delimiter!
    delimited('"', build_string, '"').parse_next(input)
}

/// A string fragment contains a fragment of a string being parsed: either
/// a non-empty Literal (a series of non-escaped characters), a single
/// parsed escaped character, or a block of escaped whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
    EscapedWS,
}

/// Combine `parse_literal`, `parse_escaped_whitespace`, and `parse_escaped_char`
/// into a `StringFragment`.
fn parse_fragment<'a, E>(input: &mut &'a str) -> Result<StringFragment<'a>, E>
where
    E: ParserError<&'a str> + FromExternalError<&'a str, std::num::ParseIntError>,
{
    alt((
        // The `map` combinator runs a parser, then applies a function to the output
        // of that parser.
        parse_literal.map(StringFragment::Literal),
        parse_escaped_char.map(StringFragment::EscapedChar),
        parse_escaped_whitespace.value(StringFragment::EscapedWS),
    ))
    .parse_next(input)
}

/// Parse a non-empty block of text that doesn't include \ or "
fn parse_literal<'a, E: ParserError<&'a str>>(input: &mut &'a str) -> Result<&'a str, E> {
    // `take_till` parses a string of 0 or more characters that aren't one of the
    // given characters.
    let not_quote_slash = take_till(1.., ['"', '\\']);

    // `verify` runs a parser, then runs a verification function on the output of
    // the parser. The verification function accepts the output only if it
    // returns true. In this case, we want to ensure that the output of take_till
    // is non-empty.
    not_quote_slash
        .verify(|s: &str| !s.is_empty())
        .parse_next(input)
}

// parser combinators are constructed from the bottom up:
// first we write parsers for the smallest elements (escaped characters),
// then combine them into larger parsers.

/// Parse an escaped character: \n, \t, \r, \u{00AC}, etc.
fn parse_escaped_char<'a, E>(input: &mut &'a str) -> Result<char, E>
where
    E: ParserError<&'a str> + FromExternalError<&'a str, std::num::ParseIntError>,
{
    preceded(
        '\\',
        // `alt` tries each parser in sequence, returning the result of
        // the first successful match
        alt((
            parse_unicode,
            // The `value` parser returns a fixed value (the first argument) if its
            // parser (the second argument) succeeds. In these cases, it looks for
            // the marker characters (n, r, t, etc) and returns the matching
            // character (\n, \r, \t, etc).
            'n'.value('\n'),
            'r'.value('\r'),
            't'.value('\t'),
            'b'.value('\u{08}'),
            'f'.value('\u{0C}'),
            '\\'.value('\\'),
            '/'.value('/'),
            '"'.value('"'),
        )),
    )
    .parse_next(input)
}

/// Parse a unicode sequence, of the form u{XXXX}, where XXXX is 1 to 6
/// hexadecimal numerals. We will combine this later with `parse_escaped_char`
/// to parse sequences like \u{00AC}.
fn parse_unicode<'a, E>(input: &mut &'a str) -> Result<char, E>
where
    E: ParserError<&'a str> + FromExternalError<&'a str, std::num::ParseIntError>,
{
    // `take_while` parses between `m` and `n` bytes (inclusive) that match
    // a predicate. `parse_hex` here parses between 1 and 6 hexadecimal numerals.
    let parse_hex = take_while(1..=6, |c: char| c.is_ascii_hexdigit());

    // `preceded` takes a prefix parser, and if it succeeds, returns the result
    // of the body parser. In this case, it parses u{XXXX}.
    let parse_delimited_hex = preceded(
        'u',
        // `delimited` is like `preceded`, but it parses both a prefix and a suffix.
        // It returns the result of the middle parser. In this case, it parses
        // {XXXX}, where XXXX is 1 to 6 hex numerals, and returns XXXX
        delimited('{', parse_hex, '}'),
    );

    // `try_map` takes the result of a parser and applies a function that returns
    // a Result. In this case we take the hex bytes from parse_hex and attempt to
    // convert them to a u32.
    let parse_u32 = parse_delimited_hex.try_map(move |hex| u32::from_str_radix(hex, 16));

    // verify_map is like try_map, but it takes an Option instead of a Result. If
    // the function returns None, verify_map returns an error. In this case, because
    // not all u32 values are valid unicode code points, we have to fallibly
    // convert to char with from_u32.
    parse_u32.verify_map(std::char::from_u32).parse_next(input)
}

/// Parse a backslash, followed by any amount of whitespace. This is used later
/// to discard any escaped whitespace.
fn parse_escaped_whitespace<'a, E: ParserError<&'a str>>(
    input: &mut &'a str,
) -> Result<&'a str, E> {
    preceded('\\', multispace1).parse_next(input)
}
