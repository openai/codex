//! Lex TOML tokens
//!
//! To get started, see [`Source::lex`][crate::Source::lex]

#[cfg(test)]
#[cfg(feature = "std")]
mod test;
mod token;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use winnow::stream::AsBStr as _;
use winnow::stream::ContainsToken as _;
use winnow::stream::FindSlice as _;
use winnow::stream::Location;
use winnow::stream::Stream as _;

use crate::Span;

pub use token::Token;
pub use token::TokenKind;

/// Lex TOML [tokens][Token]
///
/// To get started, see [`Source::lex`][crate::Source::lex]
pub struct Lexer<'i> {
    stream: Stream<'i>,
    eof: bool,
}

impl<'i> Lexer<'i> {
    pub(crate) fn new(input: &'i str) -> Self {
        let mut stream = Stream::new(input);
        if input.as_bytes().starts_with(BOM) {
            let offset = BOM.len();
            #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
            unsafe {
                stream.next_slice_unchecked(offset)
            };
            #[cfg(not(feature = "unsafe"))]
            stream.next_slice(offset);
        }
        Lexer { stream, eof: false }
    }

    #[cfg(feature = "alloc")]
    pub fn into_vec(self) -> Vec<Token> {
        #![allow(unused_qualifications)] // due to MSRV of 1.66
        let capacity = core::cmp::min(
            self.stream.len(),
            usize::MAX / core::mem::size_of::<Token>(),
        );
        let mut vec = Vec::with_capacity(capacity);
        vec.extend(self);
        vec
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(peek_byte) = self.stream.as_bstr().first() else {
            if self.eof {
                return None;
            } else {
                self.eof = true;
                let start = self.stream.current_token_start();
                let span = Span::new_unchecked(start, start);
                return Some(Token::new(TokenKind::Eof, span));
            }
        };
        Some(process_token(*peek_byte, &mut self.stream))
    }
}

const BOM: &[u8] = b"\xEF\xBB\xBF";

pub(crate) type Stream<'i> = winnow::stream::LocatingSlice<&'i str>;

fn process_token(peek_byte: u8, stream: &mut Stream<'_>) -> Token {
    let token = match peek_byte {
        b'.' => lex_ascii_char(stream, TokenKind::Dot),
        b'=' => lex_ascii_char(stream, TokenKind::Equals),
        b',' => lex_ascii_char(stream, TokenKind::Comma),
        b'[' => lex_ascii_char(stream, TokenKind::LeftSquareBracket),
        b']' => lex_ascii_char(stream, TokenKind::RightSquareBracket),
        b'{' => lex_ascii_char(stream, TokenKind::LeftCurlyBracket),
        b'}' => lex_ascii_char(stream, TokenKind::RightCurlyBracket),
        b' ' => lex_whitespace(stream),
        b'\t' => lex_whitespace(stream),
        b'#' => lex_comment(stream),
        b'\r' => lex_crlf(stream),
        b'\n' => lex_ascii_char(stream, TokenKind::Newline),
        b'\'' => {
            if stream.starts_with(ML_LITERAL_STRING_DELIM) {
                lex_ml_literal_string(stream)
            } else {
                lex_literal_string(stream)
            }
        }
        b'"' => {
            if stream.starts_with(ML_BASIC_STRING_DELIM) {
                lex_ml_basic_string(stream)
            } else {
                lex_basic_string(stream)
            }
        }
        _ => lex_atom(stream),
    };
    token
}

/// Process an ASCII character token
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
/// - `stream[0]` must be ASCII
fn lex_ascii_char(stream: &mut Stream<'_>, kind: TokenKind) -> Token {
    debug_assert!(!stream.is_empty());
    let start = stream.current_token_start();

    let offset = 1; // an ascii character
    #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(kind, span)
}

/// Process Whitespace
///
/// ```bnf
/// ;; Whitespace
///
/// ws = *wschar
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
fn lex_whitespace(stream: &mut Stream<'_>) -> Token {
    debug_assert!(!stream.is_empty());
    let start = stream.current_token_start();

    let offset = stream
        .as_bstr()
        .offset_for(|b| !WSCHAR.contains_token(b))
        .unwrap_or(stream.eof_offset());
    #[cfg(feature = "unsafe")] // SAFETY: WSCHAR ensures `offset` will be at UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::Whitespace, span)
}

/// ```bnf
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
/// ```
pub(crate) const WSCHAR: (u8, u8) = (b' ', b'\t');

/// Process Comment
///
/// ```bnf
/// ;; Comment
///
/// comment-start-symbol = %x23 ; #
/// non-ascii = %x80-D7FF / %xE000-10FFFF
/// non-eol = %x09 / %x20-7F / non-ascii
///
/// comment = comment-start-symbol *non-eol
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'#'`
fn lex_comment(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let offset = stream
        .as_bytes()
        .find_slice((b'\r', b'\n'))
        .map(|s| s.start)
        .unwrap_or_else(|| stream.eof_offset());
    #[cfg(feature = "unsafe")] // SAFETY: newlines ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::Comment, span)
}

/// `comment-start-symbol = %x23 ; #`
pub(crate) const COMMENT_START_SYMBOL: u8 = b'#';

/// Process Newline
///
/// ```bnf
/// ;; Newline
///
/// newline =  %x0A     ; LF
/// newline =/ %x0D.0A  ; CRLF
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'\r'`
fn lex_crlf(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let mut offset = '\r'.len_utf8();
    let has_lf = stream.as_bstr().get(1) == Some(&b'\n');
    if has_lf {
        offset += '\n'.len_utf8();
    }

    #[cfg(feature = "unsafe")] // SAFETY: newlines ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);
    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);

    Token::new(TokenKind::Newline, span)
}

/// Process literal string
///
/// ```bnf
/// ;; Literal String
///
/// literal-string = apostrophe *literal-char apostrophe
///
/// apostrophe = %x27 ; ' apostrophe
///
/// literal-char = %x09 / %x20-26 / %x28-7E / non-ascii
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'\''`
fn lex_literal_string(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let offset = 1; // APOSTROPHE
    #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let offset = match stream.as_bstr().find_slice((APOSTROPHE, b'\n')) {
        Some(span) => {
            if stream.as_bstr()[span.start] == APOSTROPHE {
                span.end
            } else {
                span.start
            }
        }
        None => stream.eof_offset(),
    };
    #[cfg(feature = "unsafe")]
    // SAFETY: `APOSTROPHE`/newline ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::LiteralString, span)
}

/// `apostrophe = %x27 ; ' apostrophe`
pub(crate) const APOSTROPHE: u8 = b'\'';

/// Process multi-line literal string
///
/// ```bnf
/// ;; Multiline Literal String
///
/// ml-literal-string = ml-literal-string-delim [ newline ] ml-literal-body
///                     ml-literal-string-delim
/// ml-literal-string-delim = 3apostrophe
/// ml-literal-body = *mll-content *( mll-quotes 1*mll-content ) [ mll-quotes ]
///
/// mll-content = mll-char / newline
/// mll-char = %x09 / %x20-26 / %x28-7E / non-ascii
/// mll-quotes = 1*2apostrophe
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream.starts_with(ML_LITERAL_STRING_DELIM)`
fn lex_ml_literal_string(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let offset = ML_LITERAL_STRING_DELIM.len();
    #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let offset = match stream.as_bstr().find_slice(ML_LITERAL_STRING_DELIM) {
        Some(span) => span.end,
        None => stream.eof_offset(),
    };
    #[cfg(feature = "unsafe")]
    // SAFETY: `ML_LITERAL_STRING_DELIM` ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    if stream.as_bstr().peek_token() == Some(APOSTROPHE) {
        let offset = 1;
        #[cfg(feature = "unsafe")] // SAFETY: `APOSTROPHE` ensure `offset` is along UTF-8 boundary
        unsafe {
            stream.next_slice_unchecked(offset)
        };
        #[cfg(not(feature = "unsafe"))]
        stream.next_slice(offset);

        if stream.as_bstr().peek_token() == Some(APOSTROPHE) {
            let offset = 1;
            #[cfg(feature = "unsafe")]
            // SAFETY: `APOSTROPHE` ensure `offset` is along UTF-8 boundary
            unsafe {
                stream.next_slice_unchecked(offset)
            };
            #[cfg(not(feature = "unsafe"))]
            stream.next_slice(offset);
        }
    }

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::MlLiteralString, span)
}

/// `ml-literal-string-delim = 3apostrophe`
pub(crate) const ML_LITERAL_STRING_DELIM: &str = "'''";

/// Process basic string
///
/// ```bnf
/// ;; Basic String
///
/// basic-string = quotation-mark *basic-char quotation-mark
///
/// quotation-mark = %x22            ; "
///
/// basic-char = basic-unescaped / escaped
/// basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// escaped = escape escape-seq-char
///
/// escape = %x5C                   ; \
/// escape-seq-char =  %x22         ; "    quotation mark  U+0022
/// escape-seq-char =/ %x5C         ; \    reverse solidus U+005C
/// escape-seq-char =/ %x62         ; b    backspace       U+0008
/// escape-seq-char =/ %x66         ; f    form feed       U+000C
/// escape-seq-char =/ %x6E         ; n    line feed       U+000A
/// escape-seq-char =/ %x72         ; r    carriage return U+000D
/// escape-seq-char =/ %x74         ; t    tab             U+0009
/// escape-seq-char =/ %x75 4HEXDIG ; uXXXX                U+XXXX
/// escape-seq-char =/ %x55 8HEXDIG ; UXXXXXXXX            U+XXXXXXXX
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream[0] == b'"'`
fn lex_basic_string(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let offset = 1; // QUOTATION_MARK
    #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    loop {
        // newline is present for error recovery
        match stream.as_bstr().find_slice((QUOTATION_MARK, ESCAPE, b'\n')) {
            Some(span) => {
                let found = stream.as_bstr()[span.start];
                if found == QUOTATION_MARK {
                    let offset = span.end;
                    #[cfg(feature = "unsafe")]
                    // SAFETY: `QUOTATION_MARK` ensure `offset` is along UTF-8 boundary
                    unsafe {
                        stream.next_slice_unchecked(offset)
                    };
                    #[cfg(not(feature = "unsafe"))]
                    stream.next_slice(offset);
                    break;
                } else if found == ESCAPE {
                    let offset = span.end;
                    #[cfg(feature = "unsafe")]
                    // SAFETY: `ESCAPE` / newline ensure `offset` is along UTF-8 boundary
                    unsafe {
                        stream.next_slice_unchecked(offset)
                    };
                    #[cfg(not(feature = "unsafe"))]
                    stream.next_slice(offset);

                    let peek = stream.as_bstr().peek_token();
                    match peek {
                        Some(ESCAPE) | Some(QUOTATION_MARK) => {
                            let offset = 1; // ESCAPE / QUOTATION_MARK
                            #[cfg(feature = "unsafe")]
                            #[cfg(feature = "unsafe")]
                            // SAFETY: `ESCAPE` / newline ensure `offset` is along UTF-8 boundary
                            unsafe {
                                stream.next_slice_unchecked(offset)
                            };
                            #[cfg(not(feature = "unsafe"))]
                            stream.next_slice(offset);
                        }
                        _ => {}
                    }
                    continue;
                } else if found == b'\n' {
                    let offset = span.start;
                    #[cfg(feature = "unsafe")]
                    // SAFETY: newline ensure `offset` is along UTF-8 boundary
                    unsafe {
                        stream.next_slice_unchecked(offset)
                    };
                    #[cfg(not(feature = "unsafe"))]
                    stream.next_slice(offset);
                    break;
                } else {
                    unreachable!("found `{found}`");
                }
            }
            None => {
                stream.finish();
                break;
            }
        }
    }

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::BasicString, span)
}

/// `quotation-mark = %x22            ; "`
pub(crate) const QUOTATION_MARK: u8 = b'"';

/// `escape = %x5C                   ; \`
pub(crate) const ESCAPE: u8 = b'\\';

/// Process multi-line basic string
///
/// ```bnf
/// ;; Multiline Basic String
///
/// ml-basic-string = ml-basic-string-delim [ newline ] ml-basic-body
///                   ml-basic-string-delim
/// ml-basic-string-delim = 3quotation-mark
/// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
///
/// mlb-content = mlb-char / newline / mlb-escaped-nl
/// mlb-char = mlb-unescaped / escaped
/// mlb-quotes = 1*2quotation-mark
/// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// mlb-escaped-nl = escape ws newline *( wschar / newline )
/// ```
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream.starts_with(ML_BASIC_STRING_DELIM)`
fn lex_ml_basic_string(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    let offset = ML_BASIC_STRING_DELIM.len();
    #[cfg(feature = "unsafe")] // SAFETY: only called when next character is ASCII
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    loop {
        // newline is present for error recovery
        match stream.as_bstr().find_slice((ML_BASIC_STRING_DELIM, "\\")) {
            Some(span) => {
                let found = stream.as_bstr()[span.start];
                if found == QUOTATION_MARK {
                    let offset = span.end;
                    #[cfg(feature = "unsafe")]
                    // SAFETY: `QUOTATION_MARK` ensure `offset` is along UTF-8 boundary
                    unsafe {
                        stream.next_slice_unchecked(offset)
                    };
                    #[cfg(not(feature = "unsafe"))]
                    stream.next_slice(offset);
                    break;
                } else if found == ESCAPE {
                    let offset = span.end;
                    #[cfg(feature = "unsafe")]
                    // SAFETY: `ESCAPE` ensure `offset` is along UTF-8 boundary
                    unsafe {
                        stream.next_slice_unchecked(offset)
                    };
                    #[cfg(not(feature = "unsafe"))]
                    stream.next_slice(offset);

                    let peek = stream.as_bstr().peek_token();
                    match peek {
                        Some(ESCAPE) | Some(QUOTATION_MARK) => {
                            let offset = 1; // ESCAPE / QUOTATION_MARK
                            #[cfg(feature = "unsafe")]
                            // SAFETY: `QUOTATION_MARK`/`ESCAPE` ensure `offset` is along UTF-8 boundary
                            unsafe {
                                stream.next_slice_unchecked(offset)
                            };
                            #[cfg(not(feature = "unsafe"))]
                            stream.next_slice(offset);
                        }
                        _ => {}
                    }
                    continue;
                } else {
                    unreachable!("found `{found}`");
                }
            }
            None => {
                stream.finish();
                break;
            }
        }
    }
    if stream.as_bstr().peek_token() == Some(QUOTATION_MARK) {
        let offset = 1;
        #[cfg(feature = "unsafe")]
        // SAFETY: `QUOTATION_MARK` ensure `offset` is along UTF-8 boundary
        unsafe {
            stream.next_slice_unchecked(offset)
        };
        #[cfg(not(feature = "unsafe"))]
        stream.next_slice(offset);
        if stream.as_bstr().peek_token() == Some(QUOTATION_MARK) {
            let offset = 1;
            #[cfg(feature = "unsafe")]
            // SAFETY: `QUOTATION_MARK` ensure `offset` is along UTF-8 boundary
            unsafe {
                stream.next_slice_unchecked(offset)
            };
            #[cfg(not(feature = "unsafe"))]
            stream.next_slice(offset);
        }
    }

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::MlBasicString, span)
}

/// `ml-basic-string-delim = 3quotation-mark`
pub(crate) const ML_BASIC_STRING_DELIM: &str = "\"\"\"";

/// Process Atom
///
/// This is everything else
///
/// # Safety
///
/// - `stream` must be UTF-8
/// - `stream` must be non-empty
fn lex_atom(stream: &mut Stream<'_>) -> Token {
    let start = stream.current_token_start();

    // Intentionally leaves off quotes in case the opening quote was missing
    const TOKEN_START: &[u8] = b".=,[]{} \t#\r\n";
    let offset = stream
        .as_bstr()
        .offset_for(|b| TOKEN_START.contains_token(b))
        .unwrap_or_else(|| stream.eof_offset());
    #[cfg(feature = "unsafe")] // SAFETY: `TOKEN_START` ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    };
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset);

    let end = stream.previous_token_end();
    let span = Span::new_unchecked(start, end);
    Token::new(TokenKind::Atom, span)
}
