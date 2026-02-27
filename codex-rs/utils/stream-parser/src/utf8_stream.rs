use std::error::Error;
use std::fmt;

use crate::StreamTextChunk;
use crate::StreamTextParser;

/// Error returned by [`Utf8StreamParser`] when streamed bytes are not valid UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Utf8StreamParserError {
    /// The provided bytes contain an invalid UTF-8 sequence.
    InvalidUtf8 {
        /// Byte offset in the parser's buffered bytes where decoding failed.
        valid_up_to: usize,
        /// Length in bytes of the invalid sequence.
        error_len: usize,
    },
    /// EOF was reached with a buffered partial UTF-8 code point.
    IncompleteUtf8AtEof,
}

impl fmt::Display for Utf8StreamParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 {
                valid_up_to,
                error_len,
            } => write!(
                f,
                "invalid UTF-8 in streamed bytes at offset {valid_up_to} (error length {error_len})"
            ),
            Self::IncompleteUtf8AtEof => {
                write!(f, "incomplete UTF-8 code point at end of stream")
            }
        }
    }
}

impl Error for Utf8StreamParserError {}

/// Wraps a [`StreamTextParser`] and accepts raw bytes, buffering partial UTF-8 code points.
///
/// This is useful when upstream data arrives as `&[u8]` and a code point may be split across
/// chunk boundaries (for example `0xC3` followed by `0xA9` for `é`).
#[derive(Debug)]
pub struct Utf8StreamParser<P> {
    inner: P,
    pending_utf8: Vec<u8>,
}

impl<P> Utf8StreamParser<P>
where
    P: StreamTextParser,
{
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            pending_utf8: Vec::new(),
        }
    }

    /// Feed a raw byte chunk.
    ///
    /// If the chunk contains invalid UTF-8, this returns an error and rolls back the entire
    /// pushed chunk so callers can decide how to recover without the inner parser seeing a partial
    /// prefix from that chunk.
    pub fn push_bytes(
        &mut self,
        chunk: &[u8],
    ) -> Result<StreamTextChunk<P::Extracted>, Utf8StreamParserError> {
        let old_len = self.pending_utf8.len();
        self.pending_utf8.extend_from_slice(chunk);

        match std::str::from_utf8(&self.pending_utf8) {
            Ok(text) => {
                let out = self.inner.push_str(text);
                self.pending_utf8.clear();
                Ok(out)
            }
            Err(err) => {
                if let Some(error_len) = err.error_len() {
                    self.pending_utf8.truncate(old_len);
                    return Err(Utf8StreamParserError::InvalidUtf8 {
                        valid_up_to: err.valid_up_to(),
                        error_len,
                    });
                }

                let valid_up_to = err.valid_up_to();
                if valid_up_to == 0 {
                    return Ok(StreamTextChunk::default());
                }

                let text = match std::str::from_utf8(&self.pending_utf8[..valid_up_to]) {
                    Ok(text) => text,
                    Err(prefix_err) => {
                        self.pending_utf8.truncate(old_len);
                        let error_len = prefix_err.error_len().unwrap_or(0);
                        return Err(Utf8StreamParserError::InvalidUtf8 {
                            valid_up_to: prefix_err.valid_up_to(),
                            error_len,
                        });
                    }
                };
                let out = self.inner.push_str(text);
                self.pending_utf8.drain(..valid_up_to);
                Ok(out)
            }
        }
    }

    pub fn finish(&mut self) -> Result<StreamTextChunk<P::Extracted>, Utf8StreamParserError> {
        if !self.pending_utf8.is_empty() {
            match std::str::from_utf8(&self.pending_utf8) {
                Ok(_) => {}
                Err(err) => {
                    if let Some(error_len) = err.error_len() {
                        return Err(Utf8StreamParserError::InvalidUtf8 {
                            valid_up_to: err.valid_up_to(),
                            error_len,
                        });
                    }
                    return Err(Utf8StreamParserError::IncompleteUtf8AtEof);
                }
            }
        }

        let mut out = if self.pending_utf8.is_empty() {
            StreamTextChunk::default()
        } else {
            let text = match std::str::from_utf8(&self.pending_utf8) {
                Ok(text) => text,
                Err(err) => {
                    let error_len = err.error_len().unwrap_or(0);
                    return Err(Utf8StreamParserError::InvalidUtf8 {
                        valid_up_to: err.valid_up_to(),
                        error_len,
                    });
                }
            };
            let out = self.inner.push_str(text);
            self.pending_utf8.clear();
            out
        };

        let mut tail = self.inner.finish();
        out.visible_text.push_str(&tail.visible_text);
        out.extracted.append(&mut tail.extracted);
        Ok(out)
    }

    /// Return the wrapped parser if no undecoded UTF-8 bytes are buffered.
    ///
    /// Use [`Self::finish`] first if you want to flush buffered text into the wrapped parser.
    pub fn into_inner(self) -> Result<P, Utf8StreamParserError> {
        if self.pending_utf8.is_empty() {
            return Ok(self.inner);
        }
        match std::str::from_utf8(&self.pending_utf8) {
            Ok(_) => Ok(self.inner),
            Err(err) => {
                if let Some(error_len) = err.error_len() {
                    return Err(Utf8StreamParserError::InvalidUtf8 {
                        valid_up_to: err.valid_up_to(),
                        error_len,
                    });
                }
                Err(Utf8StreamParserError::IncompleteUtf8AtEof)
            }
        }
    }

    /// Return the wrapped parser without validating or flushing buffered undecoded bytes.
    ///
    /// This may drop a partial UTF-8 code point that was buffered across chunk boundaries.
    pub fn into_inner_lossy(self) -> P {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::Utf8StreamParser;
    use super::Utf8StreamParserError;
    use crate::CitationStreamParser;
    use crate::StreamTextChunk;
    use crate::StreamTextParser;

    use pretty_assertions::assert_eq;

    fn collect_bytes(
        parser: &mut Utf8StreamParser<CitationStreamParser>,
        chunks: &[&[u8]],
    ) -> Result<StreamTextChunk<String>, Utf8StreamParserError> {
        let mut all = StreamTextChunk::default();
        for chunk in chunks {
            let next = parser.push_bytes(chunk)?;
            all.visible_text.push_str(&next.visible_text);
            all.extracted.extend(next.extracted);
        }
        let tail = parser.finish()?;
        all.visible_text.push_str(&tail.visible_text);
        all.extracted.extend(tail.extracted);
        Ok(all)
    }

    #[test]
    fn utf8_stream_parser_handles_split_code_points_across_chunks() {
        let chunks: [&[u8]; 3] = [
            b"A\xC3",
            b"\xA9<oai-mem-citation>\xE4",
            b"\xB8\xAD</oai-mem-citation>Z",
        ];

        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());
        let out = match collect_bytes(&mut parser, &chunks) {
            Ok(out) => out,
            Err(err) => panic!("valid UTF-8 stream should parse: {err}"),
        };

        assert_eq!(out.visible_text, "AéZ");
        assert_eq!(out.extracted, vec!["中".to_string()]);
    }

    #[test]
    fn utf8_stream_parser_rolls_back_on_invalid_utf8_chunk() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let first = match parser.push_bytes(&[0xC3]) {
            Ok(out) => out,
            Err(err) => panic!("leading byte may be buffered until next chunk: {err}"),
        };
        assert!(first.is_empty());

        let err = match parser.push_bytes(&[0x28]) {
            Ok(out) => panic!("invalid continuation byte should error, got output: {out:?}"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            Utf8StreamParserError::InvalidUtf8 {
                valid_up_to: 0,
                error_len: 1,
            }
        );

        let second = match parser.push_bytes(&[0xA9, b'x']) {
            Ok(out) => out,
            Err(err) => panic!("state should still allow a valid continuation: {err}"),
        };
        let tail = match parser.finish() {
            Ok(out) => out,
            Err(err) => panic!("stream should finish: {err}"),
        };

        assert_eq!(second.visible_text, "éx");
        assert!(second.extracted.is_empty());
        assert!(tail.is_empty());
    }

    #[test]
    fn utf8_stream_parser_rolls_back_entire_chunk_when_invalid_byte_follows_valid_prefix() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let err = match parser.push_bytes(b"ok\xFF") {
            Ok(out) => panic!("invalid byte should error, got output: {out:?}"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            Utf8StreamParserError::InvalidUtf8 {
                valid_up_to: 2,
                error_len: 1,
            }
        );

        let next = match parser.push_bytes(b"!") {
            Ok(out) => out,
            Err(err) => panic!("parser should recover after rollback: {err}"),
        };

        assert_eq!(next.visible_text, "!");
        assert!(next.extracted.is_empty());
    }

    #[test]
    fn utf8_stream_parser_errors_on_incomplete_code_point_at_eof() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let out = match parser.push_bytes(&[0xE2, 0x82]) {
            Ok(out) => out,
            Err(err) => panic!("partial code point should be buffered: {err}"),
        };
        assert!(out.is_empty());

        let err = match parser.finish() {
            Ok(out) => panic!("unfinished code point should error, got output: {out:?}"),
            Err(err) => err,
        };
        assert_eq!(err, Utf8StreamParserError::IncompleteUtf8AtEof);
    }

    #[test]
    fn utf8_stream_parser_into_inner_errors_when_partial_code_point_is_buffered() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let out = match parser.push_bytes(&[0xC3]) {
            Ok(out) => out,
            Err(err) => panic!("partial code point should be buffered: {err}"),
        };
        assert!(out.is_empty());

        let err = match parser.into_inner() {
            Ok(_) => panic!("buffered partial code point should be rejected"),
            Err(err) => err,
        };
        assert_eq!(err, Utf8StreamParserError::IncompleteUtf8AtEof);
    }

    #[test]
    fn utf8_stream_parser_into_inner_lossy_drops_buffered_partial_code_point() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let out = match parser.push_bytes(&[0xC3]) {
            Ok(out) => out,
            Err(err) => panic!("partial code point should be buffered: {err}"),
        };
        assert!(out.is_empty());

        let mut inner = parser.into_inner_lossy();
        let tail = inner.finish();
        assert!(tail.is_empty());
    }
}
