//! Contains low-level parsers of different XML pieces.

use crate::errors::SyntaxError;

mod element;
mod pi;

pub use element::ElementParser;
pub use pi::PiParser;

/// Used to decouple reading of data from data source and parsing XML structure from it.
/// This is a state preserved between getting chunks of bytes from the reader.
///
/// This trait is implemented for every parser that processes piece of XML grammar.
pub trait Parser {
    /// Process new data and try to determine end of the parsed thing.
    ///
    /// Returns position of the end of thing in `bytes` in case of successful search
    /// and `None` otherwise.
    ///
    /// # Parameters
    /// - `bytes`: a slice to find the end of a thing.
    ///   Should contain text in ASCII-compatible encoding
    fn feed(&mut self, bytes: &[u8]) -> Option<usize>;

    /// Returns parse error produced by this parser in case of reaching end of
    /// input without finding the end of a parsed thing.
    fn eof_error() -> SyntaxError;
}
