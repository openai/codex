#[cfg(feature = "encoding")]
use encoding_rs::UTF_8;

use crate::encoding::Decoder;
use crate::errors::{Error, IllFormedError, Result, SyntaxError};
use crate::events::{BytesCData, BytesDecl, BytesEnd, BytesPI, BytesStart, BytesText, Event};
#[cfg(feature = "encoding")]
use crate::reader::EncodingRef;
use crate::reader::{BangType, Config, ParseState};
use crate::utils::{is_whitespace, name_len};

/// A struct that holds a current reader state and a parser configuration.
/// It is independent on a way of reading data: the reader feed data into it and
/// get back produced [`Event`]s.
#[derive(Clone, Debug)]
pub(super) struct ReaderState {
    /// Number of bytes read from the source of data since the reader was created
    pub offset: u64,
    /// A snapshot of an `offset` of the last error returned. It can be less than
    /// `offset`, because some errors conveniently report at earlier position,
    /// and changing `offset` is not possible, because `Error::IllFormed` errors
    /// are recoverable.
    pub last_error_offset: u64,
    /// Defines how to process next byte
    pub state: ParseState,
    /// User-defined settings that affect parsing
    pub config: Config,
    /// All currently Started elements which didn't have a matching
    /// End element yet.
    ///
    /// For an XML
    ///
    /// ```xml
    /// <root><one/><inner attr="value">|<tag></inner></root>
    /// ```
    /// when cursor at the `|` position buffer contains:
    ///
    /// ```text
    /// rootinner
    /// ^   ^
    /// ```
    ///
    /// The `^` symbols shows which positions stored in the [`Self::opened_starts`]
    /// (0 and 4 in that case).
    opened_buffer: Vec<u8>,
    /// Opened name start indexes into [`Self::opened_buffer`]. See documentation
    /// for that field for details
    opened_starts: Vec<usize>,

    #[cfg(feature = "encoding")]
    /// Reference to the encoding used to read an XML
    pub encoding: EncodingRef,
}

impl ReaderState {
    /// Trims end whitespaces from `bytes`, if required, and returns a text event.
    ///
    /// # Parameters
    /// - `bytes`: data from the start of stream to the first `<` or from `>` to `<`
    pub fn emit_text<'b>(&mut self, bytes: &'b [u8]) -> BytesText<'b> {
        let mut content = bytes;

        if self.config.trim_text_end {
            // Skip the ending '<'
            let len = bytes
                .iter()
                .rposition(|&b| !is_whitespace(b))
                .map_or(0, |p| p + 1);
            content = &bytes[..len];
        }
        BytesText::wrap(content, self.decoder())
    }

    /// Returns `Comment`, `CData` or `DocType` event.
    ///
    /// `buf` contains data between `<` and `>`:
    /// - CDATA: `![CDATA[...]]`
    /// - Comment: `!--...--`
    /// - Doctype (uppercase): `!D...`
    /// - Doctype (lowercase): `!d...`
    pub fn emit_bang<'b>(&mut self, bang_type: BangType, buf: &'b [u8]) -> Result<Event<'b>> {
        debug_assert_eq!(
            buf.first(),
            Some(&b'!'),
            "CDATA, comment or DOCTYPE should start from '!'"
        );

        let uncased_starts_with = |string: &[u8], prefix: &[u8]| {
            string.len() >= prefix.len() && string[..prefix.len()].eq_ignore_ascii_case(prefix)
        };

        let len = buf.len();
        match bang_type {
            BangType::Comment if buf.starts_with(b"!--") => {
                debug_assert!(buf.ends_with(b"--"));
                if self.config.check_comments {
                    // search if '--' not in comments
                    let mut haystack = &buf[3..len - 2];
                    let mut off = 0;
                    while let Some(p) = memchr::memchr(b'-', haystack) {
                        off += p + 1;
                        // if next byte after `-` is also `-`, return an error
                        if buf[3 + off] == b'-' {
                            // Explanation of the magic:
                            //
                            // - `self.offset`` just after `>`,
                            // - `buf` contains `!-- con--tent --`
                            // - `p` is counted from byte after `<!--`
                            //
                            // <!-- con--tent -->:
                            //  ~~~~~~~~~~~~~~~~ : - buf
                            //   : ===========   : - zone of search (possible values of `p`)
                            //   : |---p         : - p is counted from | (| is 0)
                            //   : :   :         ^ - self.offset
                            //   ^ :   :           - self.offset - len
                            //     ^   :           - self.offset - len + 2
                            //         ^           - self.offset - len + 2 + p
                            self.last_error_offset = self.offset - len as u64 + 2 + p as u64;
                            return Err(Error::IllFormed(IllFormedError::DoubleHyphenInComment));
                        }
                        // Continue search after single `-` (+1 to skip it)
                        haystack = &haystack[p + 1..];
                    }
                }
                Ok(Event::Comment(BytesText::wrap(
                    // Cut of `!--` and `--` from start and end
                    &buf[3..len - 2],
                    self.decoder(),
                )))
            }
            // XML requires uppercase only:
            // https://www.w3.org/TR/xml11/#sec-cdata-sect
            // Even HTML5 required uppercase only:
            // https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state
            BangType::CData if buf.starts_with(b"![CDATA[") => {
                debug_assert!(buf.ends_with(b"]]"));
                Ok(Event::CData(BytesCData::wrap(
                    // Cut of `![CDATA[` and `]]` from start and end
                    &buf[8..len - 2],
                    self.decoder(),
                )))
            }
            // XML requires uppercase only, but we will check that on validation stage:
            // https://www.w3.org/TR/xml11/#sec-prolog-dtd
            // HTML5 allows mixed case for doctype declarations:
            // https://html.spec.whatwg.org/multipage/parsing.html#markup-declaration-open-state
            BangType::DocType(0) if uncased_starts_with(buf, b"!DOCTYPE") => {
                match buf[8..].iter().position(|&b| !is_whitespace(b)) {
                    Some(start) => Ok(Event::DocType(BytesText::wrap(
                        // Cut of `!DOCTYPE` and any number of spaces from start
                        &buf[8 + start..],
                        self.decoder(),
                    ))),
                    None => {
                        // Because we here, we at least read `<!DOCTYPE>` and offset after `>`.
                        // We want report error at place where name is expected - this is just
                        // before `>`
                        self.last_error_offset = self.offset - 1;
                        return Err(Error::IllFormed(IllFormedError::MissingDoctypeName));
                    }
                }
            }
            _ => {
                // <!....>
                //  ^^^^^ - `buf` does not contain `<` and `>`, but `self.offset` is after `>`.
                // ^------- We report error at that position, so we need to subtract 2 and buf len
                self.last_error_offset = self.offset - len as u64 - 2;
                Err(bang_type.to_err().into())
            }
        }
    }

    /// Wraps content of `buf` into the [`Event::End`] event. Does the check that
    /// end name matches the last opened start name if `self.config.check_end_names` is set.
    ///
    /// `buf` contains data between `<` and `>`, for example `/tag`.
    pub fn emit_end<'b>(&mut self, buf: &'b [u8]) -> Result<Event<'b>> {
        debug_assert_eq!(
            buf.first(),
            Some(&b'/'),
            "closing tag should start from '/'"
        );

        // Strip the `/` character. `content` contains data between `</` and `>`
        let content = &buf[1..];
        // XML standard permits whitespaces after the markup name in closing tags.
        // Let's strip them from the buffer before comparing tag names.
        let name = if self.config.trim_markup_names_in_closing_tags {
            if let Some(pos_end_name) = content.iter().rposition(|&b| !is_whitespace(b)) {
                &content[..pos_end_name + 1]
            } else {
                content
            }
        } else {
            content
        };

        let decoder = self.decoder();

        // Get the index in self.opened_buffer of the name of the last opened tag
        match self.opened_starts.pop() {
            Some(start) => {
                if self.config.check_end_names {
                    let expected = &self.opened_buffer[start..];
                    if name != expected {
                        let expected = decoder.decode(expected).unwrap_or_default().into_owned();
                        // #513: In order to allow error recovery we should drop content of the buffer
                        self.opened_buffer.truncate(start);

                        // Report error at start of the end tag at `<` character
                        // -2 for `<` and `>`
                        self.last_error_offset = self.offset - buf.len() as u64 - 2;
                        return Err(Error::IllFormed(IllFormedError::MismatchedEndTag {
                            expected,
                            found: decoder.decode(name).unwrap_or_default().into_owned(),
                        }));
                    }
                }

                self.opened_buffer.truncate(start);
            }
            None => {
                if !self.config.allow_unmatched_ends {
                    // Report error at start of the end tag at `<` character
                    // -2 for `<` and `>`
                    self.last_error_offset = self.offset - buf.len() as u64 - 2;
                    return Err(Error::IllFormed(IllFormedError::UnmatchedEndTag(
                        decoder.decode(name).unwrap_or_default().into_owned(),
                    )));
                }
            }
        }

        Ok(Event::End(BytesEnd::wrap(name.into())))
    }

    /// `buf` contains data between `<` and `>` and the first byte is `?`.
    /// `self.offset` already after the `>`
    ///
    /// Returns `Decl` or `PI` event
    pub fn emit_question_mark<'b>(&mut self, buf: &'b [u8]) -> Result<Event<'b>> {
        debug_assert!(buf.len() > 0);
        debug_assert_eq!(buf[0], b'?');

        let len = buf.len();
        // We accept at least <??>
        //                     ~~ - len = 2
        if len > 1 && buf[len - 1] == b'?' {
            // Cut of `?` and `?` from start and end
            let content = &buf[1..len - 1];
            let len = content.len();

            if content.starts_with(b"xml") && (len == 3 || is_whitespace(content[3])) {
                let event = BytesDecl::from_start(BytesStart::wrap(content, 3));

                // Try getting encoding from the declaration event
                #[cfg(feature = "encoding")]
                if self.encoding.can_be_refined() {
                    if let Some(encoding) = event.encoder() {
                        self.encoding = EncodingRef::XmlDetected(encoding);
                    }
                }

                Ok(Event::Decl(event))
            } else {
                Ok(Event::PI(BytesPI::wrap(content, name_len(content))))
            }
        } else {
            // <?....EOF
            //  ^^^^^ - `buf` does not contains `<`, but we want to report error at `<`,
            //          so we move offset to it (-2 for `<` and `>`)
            self.last_error_offset = self.offset - len as u64 - 2;
            Err(Error::Syntax(SyntaxError::UnclosedPIOrXmlDecl))
        }
    }

    /// Converts content of a tag to a `Start` or an `Empty` event
    ///
    /// # Parameters
    /// - `content`: Content of a tag between `<` and `>`
    pub fn emit_start<'b>(&mut self, content: &'b [u8]) -> Event<'b> {
        if let Some(content) = content.strip_suffix(b"/") {
            // This is self-closed tag `<something/>`
            let event = BytesStart::wrap(content, name_len(content));

            if self.config.expand_empty_elements {
                self.state = ParseState::InsideEmpty;
                self.opened_starts.push(self.opened_buffer.len());
                self.opened_buffer.extend(event.name().as_ref());
                Event::Start(event)
            } else {
                Event::Empty(event)
            }
        } else {
            let event = BytesStart::wrap(content, name_len(content));

            // #514: Always store names event when .check_end_names == false,
            // because checks can be temporary disabled and when they would be
            // enabled, we should have that information
            self.opened_starts.push(self.opened_buffer.len());
            self.opened_buffer.extend(event.name().as_ref());
            Event::Start(event)
        }
    }

    #[inline]
    pub fn close_expanded_empty(&mut self) -> BytesEnd<'static> {
        self.state = ParseState::InsideText;
        let name = self
            .opened_buffer
            .split_off(self.opened_starts.pop().unwrap());
        BytesEnd::wrap(name.into())
    }

    /// Get the decoder, used to decode bytes, read by this reader, to the strings.
    ///
    /// If [`encoding`] feature is enabled, the used encoding may change after
    /// parsing the XML declaration, otherwise encoding is fixed to UTF-8.
    ///
    /// If [`encoding`] feature is enabled and no encoding is specified in declaration,
    /// defaults to UTF-8.
    ///
    /// [`encoding`]: ../../index.html#encoding
    pub const fn decoder(&self) -> Decoder {
        Decoder {
            #[cfg(feature = "encoding")]
            encoding: self.encoding.encoding(),
        }
    }
}

impl Default for ReaderState {
    fn default() -> Self {
        Self {
            offset: 0,
            last_error_offset: 0,
            state: ParseState::Init,
            config: Config::default(),
            opened_buffer: Vec::new(),
            opened_starts: Vec::new(),

            #[cfg(feature = "encoding")]
            encoding: EncodingRef::Implicit(UTF_8),
        }
    }
}
