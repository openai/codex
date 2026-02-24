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

    pub fn push_bytes(
        &mut self,
        chunk: &[u8],
    ) -> Result<StreamTextChunk<P::Extracted>, Utf8StreamParserError> {
        let old_len = self.pending_utf8.len();
        self.pending_utf8.extend_from_slice(chunk);

        match std::str::from_utf8(&self.pending_utf8) {
            Ok(_) => {
                let out = {
                    let text = std::str::from_utf8(&self.pending_utf8)
                        .expect("pending_utf8 was validated by from_utf8");
                    self.inner.push_str(text)
                };
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

                let out = {
                    let text = std::str::from_utf8(&self.pending_utf8[..valid_up_to])
                        .expect("valid_up_to from Utf8Error is always on a char boundary");
                    self.inner.push_str(text)
                };
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
            let out = {
                let text = std::str::from_utf8(&self.pending_utf8)
                    .expect("pending_utf8 was validated by from_utf8");
                self.inner.push_str(text)
            };
            self.pending_utf8.clear();
            out
        };

        let mut tail = self.inner.finish();
        out.visible_text.push_str(&tail.visible_text);
        out.extracted.append(&mut tail.extracted);
        Ok(out)
    }

    pub fn into_inner(self) -> P {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::Utf8StreamParser;
    use super::Utf8StreamParserError;
    use crate::CitationStreamParser;
    use crate::StreamTextChunk;

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
        let text = "Aé<citation>中</citation>Z";
        let bytes = text.as_bytes();
        let e_idx = text.find('é').expect("test string contains é");
        let cjk_idx = text.find('中').expect("test string contains 中");

        let chunks = [
            &bytes[..e_idx + 1],
            &bytes[e_idx + 1..cjk_idx + 1],
            &bytes[cjk_idx + 1..],
        ];

        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());
        let out = collect_bytes(&mut parser, &chunks).expect("valid UTF-8 stream should parse");

        assert_eq!(out.visible_text, "AéZ");
        assert_eq!(out.extracted, vec!["中".to_string()]);
    }

    #[test]
    fn utf8_stream_parser_rolls_back_on_invalid_utf8_chunk() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let first = parser
            .push_bytes(&[0xC3])
            .expect("leading byte may be buffered until next chunk");
        assert!(first.is_empty());

        let err = parser
            .push_bytes(&[0x28])
            .expect_err("invalid continuation byte should error");
        assert_eq!(
            err,
            Utf8StreamParserError::InvalidUtf8 {
                valid_up_to: 0,
                error_len: 1,
            }
        );

        let second = parser
            .push_bytes(&[0xA9, b'x'])
            .expect("state should still allow a valid continuation");
        let tail = parser.finish().expect("stream should finish");

        assert_eq!(second.visible_text, "éx");
        assert!(second.extracted.is_empty());
        assert!(tail.is_empty());
    }

    #[test]
    fn utf8_stream_parser_errors_on_incomplete_code_point_at_eof() {
        let mut parser = Utf8StreamParser::new(CitationStreamParser::new());

        let out = parser
            .push_bytes(&[0xE2, 0x82])
            .expect("partial code point should be buffered");
        assert!(out.is_empty());

        let err = parser
            .finish()
            .expect_err("unfinished code point should error");
        assert_eq!(err, Utf8StreamParserError::IncompleteUtf8AtEof);
    }
}
