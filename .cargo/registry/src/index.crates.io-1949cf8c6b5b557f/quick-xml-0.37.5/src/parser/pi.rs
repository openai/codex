//! Contains a parser for an XML processing instruction.

use crate::errors::SyntaxError;
use crate::parser::Parser;

/// A parser that search a `?>` sequence in the slice.
///
/// To use a parser create an instance of parser and [`feed`] data into it.
/// After successful search the parser will return [`Some`] with position where
/// processing instruction is ended (the position after `?>`). If search was
/// unsuccessful, a [`None`] will be returned. You typically would expect positive
/// result of search, so that you should feed new data until you get it.
///
/// NOTE: after successful match the parser does not returned to the initial
/// state and should not be used anymore. Create a new parser if you want to perform
/// new search.
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::parser::{Parser, PiParser};
///
/// let mut parser = PiParser::default();
///
/// // Parse `<?instruction with = 'some > and ?' inside?>and the text follow...`
/// // splitted into three chunks
/// assert_eq!(parser.feed(b"<?instruction"), None);
/// // ...get new chunk of data
/// assert_eq!(parser.feed(b" with = 'some > and ?"), None);
/// // ...get another chunk of data
/// assert_eq!(parser.feed(b"' inside?>and the text follow..."), Some(9));
/// //                       ^        ^
/// //                       0        9
/// ```
///
/// [`feed`]: Self::feed()
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PiParser(
    /// A flag that indicates was the `bytes` in the previous attempt to find the
    /// end ended with `?`.
    pub bool,
);

impl Parser for PiParser {
    /// Determines the end position of a processing instruction in the provided slice.
    /// Processing instruction ends on the first occurrence of `?>` which cannot be
    /// escaped.
    ///
    /// Returns position after the `?>` or `None` if such sequence was not found.
    ///
    /// [Section 2.6]: Parameter entity references MUST NOT be recognized within
    /// processing instructions, so parser do not search for them.
    ///
    /// # Parameters
    /// - `bytes`: a slice to find the end of a processing instruction.
    ///   Should contain text in ASCII-compatible encoding
    ///
    /// [Section 2.6]: https://www.w3.org/TR/xml11/#sec-pi
    #[inline]
    fn feed(&mut self, bytes: &[u8]) -> Option<usize> {
        for i in memchr::memchr_iter(b'>', bytes) {
            match i {
                0 if self.0 => return Some(0),
                // If the previous byte is `?`, then we found `?>`
                i if i > 0 && bytes[i - 1] == b'?' => return Some(i),
                _ => {}
            }
        }
        self.0 = bytes.last().copied() == Some(b'?');
        None
    }

    #[inline]
    fn eof_error() -> SyntaxError {
        SyntaxError::UnclosedPIOrXmlDecl
    }
}

#[test]
fn pi() {
    use pretty_assertions::assert_eq;

    /// Returns `Ok(pos)` with the position in the buffer where processing
    /// instruction is ended.
    ///
    /// Returns `Err(internal_state)` if parsing is not done yet.
    fn parse_pi(bytes: &[u8], had_question_mark: bool) -> Result<usize, bool> {
        let mut parser = PiParser(had_question_mark);
        match parser.feed(bytes) {
            Some(i) => Ok(i),
            None => Err(parser.0),
        }
    }

    // Comments shows which character was seen the last before calling `feed`.
    // `x` means any character, pipe denotes start of the buffer that passed to `feed`

    assert_eq!(parse_pi(b"", false), Err(false)); // x|
    assert_eq!(parse_pi(b"", true), Err(false)); // ?|

    assert_eq!(parse_pi(b"?", false), Err(true)); // x|?
    assert_eq!(parse_pi(b"?", true), Err(true)); // ?|?

    assert_eq!(parse_pi(b">", false), Err(false)); // x|>
    assert_eq!(parse_pi(b">", true), Ok(0)); // ?|>

    assert_eq!(parse_pi(b"?>", false), Ok(1)); // x|?>
    assert_eq!(parse_pi(b"?>", true), Ok(1)); // ?|?>

    assert_eq!(parse_pi(b">?>", false), Ok(2)); // x|>?>
    assert_eq!(parse_pi(b">?>", true), Ok(0)); // ?|>?>
}
