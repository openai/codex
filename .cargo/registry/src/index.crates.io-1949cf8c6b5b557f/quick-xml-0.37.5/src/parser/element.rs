//! Contains a parser for an XML element.

use crate::errors::SyntaxError;
use crate::parser::Parser;

/// A parser that search a `>` symbol in the slice outside of quoted regions.
///
/// The parser considers two quoted regions: a double-quoted (`"..."`) and
/// a single-quoted (`'...'`) region. Matches found inside those regions are not
/// considered as results. Each region starts and ends by its quote symbol,
/// which cannot be escaped (but can be encoded as XML character entity or named
/// entity. Anyway, that encoding does not contain literal quotes).
///
/// To use a parser create an instance of parser and [`feed`] data into it.
/// After successful search the parser will return [`Some`] with position of
/// found symbol. If search is unsuccessful, a [`None`] will be returned. You
/// typically would expect positive result of search, so that you should feed
/// new data until you get it.
///
/// NOTE: after successful match the parser does not returned to the initial
/// state and should not be used anymore. Create a new parser if you want to perform
/// new search.
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::parser::{ElementParser, Parser};
///
/// let mut parser = ElementParser::default();
///
/// // Parse `<my-element  with = 'some > inside'>and the text follow...`
/// // splitted into three chunks
/// assert_eq!(parser.feed(b"<my-element"), None);
/// // ...get new chunk of data
/// assert_eq!(parser.feed(b" with = 'some >"), None);
/// // ...get another chunk of data
/// assert_eq!(parser.feed(b" inside'>and the text follow..."), Some(8));
/// //                       ^       ^
/// //                       0       8
/// ```
///
/// [`feed`]: Self::feed()
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementParser {
    /// The initial state (inside element, but outside of attribute value).
    Outside,
    /// Inside a single-quoted region (`'...'`).
    SingleQ,
    /// Inside a double-quoted region (`"..."`).
    DoubleQ,
}

impl Parser for ElementParser {
    /// Returns number of consumed bytes or `None` if `>` was not found in `bytes`.
    #[inline]
    fn feed(&mut self, bytes: &[u8]) -> Option<usize> {
        for i in memchr::memchr3_iter(b'>', b'\'', b'"', bytes) {
            *self = match (*self, bytes[i]) {
                // only allowed to match `>` while we are in state `Outside`
                (Self::Outside, b'>') => return Some(i),
                (Self::Outside, b'\'') => Self::SingleQ,
                (Self::Outside, b'\"') => Self::DoubleQ,

                // the only end_byte that gets us out if the same character
                (Self::SingleQ, b'\'') | (Self::DoubleQ, b'"') => Self::Outside,

                // all other bytes: no state change
                _ => continue,
            };
        }
        None
    }

    #[inline]
    fn eof_error() -> SyntaxError {
        SyntaxError::UnclosedTag
    }
}

impl Default for ElementParser {
    #[inline]
    fn default() -> Self {
        Self::Outside
    }
}

#[test]
fn parse() {
    use pretty_assertions::assert_eq;
    use ElementParser::*;

    /// Returns `Ok(pos)` with the position in the buffer where element is ended.
    ///
    /// Returns `Err(internal_state)` if parsing does not done yet.
    fn parse_element(bytes: &[u8], mut parser: ElementParser) -> Result<usize, ElementParser> {
        match parser.feed(bytes) {
            Some(i) => Ok(i),
            None => Err(parser),
        }
    }

    assert_eq!(parse_element(b"", Outside), Err(Outside));
    assert_eq!(parse_element(b"", SingleQ), Err(SingleQ));
    assert_eq!(parse_element(b"", DoubleQ), Err(DoubleQ));

    assert_eq!(parse_element(b"'", Outside), Err(SingleQ));
    assert_eq!(parse_element(b"'", SingleQ), Err(Outside));
    assert_eq!(parse_element(b"'", DoubleQ), Err(DoubleQ));

    assert_eq!(parse_element(b"\"", Outside), Err(DoubleQ));
    assert_eq!(parse_element(b"\"", SingleQ), Err(SingleQ));
    assert_eq!(parse_element(b"\"", DoubleQ), Err(Outside));

    assert_eq!(parse_element(b">", Outside), Ok(0));
    assert_eq!(parse_element(b">", SingleQ), Err(SingleQ));
    assert_eq!(parse_element(b">", DoubleQ), Err(DoubleQ));

    assert_eq!(parse_element(b"''>", Outside), Ok(2));
    assert_eq!(parse_element(b"''>", SingleQ), Err(SingleQ));
    assert_eq!(parse_element(b"''>", DoubleQ), Err(DoubleQ));
}
