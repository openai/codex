//! Enables base64'd output anywhere you might use a `Display` implementation, like a format string.
//!
//! ```
//! use base64::{display::Base64Display, engine::general_purpose::STANDARD};
//!
//! let data = vec![0x0, 0x1, 0x2, 0x3];
//! let wrapper = Base64Display::new(&data, &STANDARD);
//!
//! assert_eq!("base64: AAECAw==", format!("base64: {}", wrapper));
//! ```

use super::chunked_encoder::ChunkedEncoder;
use crate::engine::Engine;
use core::fmt::{Display, Formatter};
use core::{fmt, str};

/// A convenience wrapper for base64'ing bytes into a format string without heap allocation.
pub struct Base64Display<'a, 'e, E: Engine> {
    bytes: &'a [u8],
    chunked_encoder: ChunkedEncoder<'e, E>,
}

impl<'a, 'e, E: Engine> Base64Display<'a, 'e, E> {
    /// Create a `Base64Display` with the provided engine.
    pub fn new(bytes: &'a [u8], engine: &'e E) -> Base64Display<'a, 'e, E> {
        Base64Display {
            bytes,
            chunked_encoder: ChunkedEncoder::new(engine),
        }
    }
}

impl<'a, 'e, E: Engine> Display for Base64Display<'a, 'e, E> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let mut sink = FormatterSink { f: formatter };
        self.chunked_encoder.encode(self.bytes, &mut sink)
    }
}

struct FormatterSink<'a, 'b: 'a> {
    f: &'a mut Formatter<'b>,
}

impl<'a, 'b: 'a> super::chunked_encoder::Sink for FormatterSink<'a, 'b> {
    type Error = fmt::Error;

    fn write_encoded_bytes(&mut self, encoded: &[u8]) -> Result<(), Self::Error> {
        // Avoid unsafe. If max performance is needed, write your own display wrapper that uses
        // unsafe here to gain about 10-15%.
        self.f
            .write_str(str::from_utf8(encoded).expect("base64 data was not utf8"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::chunked_encoder::tests::{
        chunked_encode_matches_normal_encode_random, SinkTestHelper,
    };
    use super::*;
    use crate::engine::general_purpose::STANDARD;

    #[test]
    fn basic_display() {
        assert_eq!(
            "~$Zm9vYmFy#*",
            format!("~${}#*", Base64Display::new(b"foobar", &STANDARD))
        );
        assert_eq!(
            "~$Zm9vYmFyZg==#*",
            format!("~${}#*", Base64Display::new(b"foobarf", &STANDARD))
        );
    }

    #[test]
    fn display_encode_matches_normal_encode() {
        let helper = DisplaySinkTestHelper;
        chunked_encode_matches_normal_encode_random(&helper);
    }

    struct DisplaySinkTestHelper;

    impl SinkTestHelper for DisplaySinkTestHelper {
        fn encode_to_string<E: Engine>(&self, engine: &E, bytes: &[u8]) -> String {
            format!("{}", Base64Display::new(bytes, engine))
        }
    }
}
