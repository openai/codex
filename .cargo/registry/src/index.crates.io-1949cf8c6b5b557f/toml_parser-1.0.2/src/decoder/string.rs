use core::ops::RangeInclusive;

use winnow::stream::ContainsToken as _;
use winnow::stream::Offset as _;
use winnow::stream::Stream as _;

use crate::decoder::StringBuilder;
use crate::lexer::APOSTROPHE;
use crate::lexer::ML_BASIC_STRING_DELIM;
use crate::lexer::ML_LITERAL_STRING_DELIM;
use crate::lexer::QUOTATION_MARK;
use crate::lexer::WSCHAR;
use crate::ErrorSink;
use crate::Expected;
use crate::ParseError;
use crate::Raw;
use crate::Span;

const ALLOCATION_ERROR: &str = "could not allocate for string";

/// Parse literal string
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
pub(crate) fn decode_literal_string<'i>(
    raw: Raw<'i>,
    output: &mut dyn StringBuilder<'i>,
    error: &mut dyn ErrorSink,
) {
    const INVALID_STRING: &str = "invalid literal string";

    output.clear();

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(APOSTROPHE as char) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("'")])
                .with_unexpected(Span::new_unchecked(0, 0)),
        );
        s
    };
    let s = if let Some(stripped) = s.strip_suffix(APOSTROPHE as char) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("'")])
                .with_unexpected(Span::new_unchecked(raw.len(), raw.len())),
        );
        s
    };

    for (i, b) in s.as_bytes().iter().enumerate() {
        if !LITERAL_CHAR.contains_token(b) {
            let offset = (&s.as_bytes()[i..]).offset_from(&raw.as_bytes());
            error.report_error(
                ParseError::new(INVALID_STRING)
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[Expected::Description("non-single-quote visible characters")])
                    .with_unexpected(Span::new_unchecked(offset, offset)),
            );
        }
    }

    if !output.push_str(s) {
        error.report_error(
            ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(0, raw.len())),
        );
    }
}

/// `literal-char = %x09 / %x20-26 / %x28-7E / non-ascii`
const LITERAL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

/// `non-ascii = %x80-D7FF / %xE000-10FFFF`
/// - ASCII is 0xxxxxxx
/// - First byte for UTF-8 is 11xxxxxx
/// - Subsequent UTF-8 bytes are 10xxxxxx
const NON_ASCII: RangeInclusive<u8> = 0x80..=0xff;

/// Parse multi-line literal string
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
/// mll-quotes = 1*2apostrophe
/// ```
pub(crate) fn decode_ml_literal_string<'i>(
    raw: Raw<'i>,
    output: &mut dyn StringBuilder<'i>,
    error: &mut dyn ErrorSink,
) {
    const INVALID_STRING: &str = "invalid multi-line literal string";
    output.clear();

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(ML_LITERAL_STRING_DELIM) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("'")])
                .with_unexpected(Span::new_unchecked(0, 0)),
        );
        s
    };
    let s = strip_start_newline(s);
    let s = if let Some(stripped) = s.strip_suffix(ML_LITERAL_STRING_DELIM) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("'")])
                .with_unexpected(Span::new_unchecked(raw.len(), raw.len())),
        );
        s.trim_end_matches('\'')
    };

    for (i, b) in s.as_bytes().iter().enumerate() {
        if *b == b'\'' || *b == b'\n' {
        } else if *b == b'\r' {
            if s.as_bytes().get(i + 1) != Some(&b'\n') {
                let offset = (&s.as_bytes()[i + 1..]).offset_from(&raw.as_bytes());
                error.report_error(
                    ParseError::new("carriage return must be followed by newline")
                        .with_context(Span::new_unchecked(0, raw.len()))
                        .with_expected(&[Expected::Literal("\n")])
                        .with_unexpected(Span::new_unchecked(offset, offset)),
                );
            }
        } else if !MLL_CHAR.contains_token(b) {
            let offset = (&s.as_bytes()[i..]).offset_from(&raw.as_bytes());
            error.report_error(
                ParseError::new(INVALID_STRING)
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[Expected::Description("non-single-quote characters")])
                    .with_unexpected(Span::new_unchecked(offset, offset)),
            );
        }
    }

    if !output.push_str(s) {
        error.report_error(
            ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(0, raw.len())),
        );
    }
}

/// `mll-char = %x09 / %x20-26 / %x28-7E / non-ascii`
const MLL_CHAR: (
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (0x9, 0x20..=0x26, 0x28..=0x7E, NON_ASCII);

/// Parse basic string
///
/// ```bnf
/// ;; Basic String
///
/// basic-string = quotation-mark *basic-char quotation-mark
///
/// basic-char = basic-unescaped / escaped
///
/// escaped = escape escape-seq-char
/// ```
pub(crate) fn decode_basic_string<'i>(
    raw: Raw<'i>,
    output: &mut dyn StringBuilder<'i>,
    error: &mut dyn ErrorSink,
) {
    const INVALID_STRING: &str = "invalid basic string";
    output.clear();

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(QUOTATION_MARK as char) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("\"")])
                .with_unexpected(Span::new_unchecked(0, 0)),
        );
        s
    };
    let mut s = if let Some(stripped) = s.strip_suffix(QUOTATION_MARK as char) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("\"")])
                .with_unexpected(Span::new_unchecked(raw.len(), raw.len())),
        );
        s
    };

    let segment = basic_unescaped(&mut s);
    if !output.push_str(segment) {
        error.report_error(
            ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(0, raw.len())),
        );
    }
    while !s.is_empty() {
        if s.starts_with("\\") {
            let _ = s.next_token();

            let c = escape_seq_char(&mut s, raw, error);
            if !output.push_char(c) {
                error.report_error(
                    ParseError::new(ALLOCATION_ERROR)
                        .with_unexpected(Span::new_unchecked(0, raw.len())),
                );
            }
        } else {
            let invalid = basic_invalid(&mut s);
            let start = invalid.offset_from(&raw.as_str());
            let end = start + invalid.len();
            error.report_error(
                ParseError::new(INVALID_STRING)
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[
                        Expected::Description("non-double-quote visible characters"),
                        Expected::Literal("\\"),
                    ])
                    .with_unexpected(Span::new_unchecked(start, end)),
            );
            let _ = output.push_str(invalid);
        }

        let segment = basic_unescaped(&mut s);
        if !output.push_str(segment) {
            let start = segment.offset_from(&raw.as_str());
            let end = start + segment.len();
            error.report_error(
                ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(start, end)),
            );
        }
    }
}

/// `basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
fn basic_unescaped<'i>(stream: &mut &'i str) -> &'i str {
    let offset = stream
        .as_bytes()
        .offset_for(|b| !BASIC_UNESCAPED.contains_token(b))
        .unwrap_or(stream.len());
    #[cfg(feature = "unsafe")] // SAFETY: BASIC_UNESCAPED ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    }
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset)
}

fn basic_invalid<'i>(stream: &mut &'i str) -> &'i str {
    let offset = stream
        .as_bytes()
        .offset_for(|b| (BASIC_UNESCAPED, ESCAPE).contains_token(b))
        .unwrap_or(stream.len());
    #[cfg(feature = "unsafe")] // SAFETY: BASIC_UNESCAPED ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    }
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset)
}

/// `basic-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
#[allow(clippy::type_complexity)]
const BASIC_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

/// `escape = %x5C                    ; \`
const ESCAPE: u8 = b'\\';

/// ```bnf
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
fn escape_seq_char(stream: &mut &str, raw: Raw<'_>, error: &mut dyn ErrorSink) -> char {
    const EXPECTED_ESCAPES: &[Expected] = &[
        Expected::Literal("b"),
        Expected::Literal("f"),
        Expected::Literal("n"),
        Expected::Literal("r"),
        Expected::Literal("\\"),
        Expected::Literal("\""),
        Expected::Literal("u"),
        Expected::Literal("U"),
    ];

    let start = stream.checkpoint();
    let Some(id) = stream.next_token() else {
        let offset = stream.offset_from(&raw.as_str());
        error.report_error(
            ParseError::new("missing escaped value")
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(EXPECTED_ESCAPES)
                .with_unexpected(Span::new_unchecked(offset, offset)),
        );
        return '\\';
    };
    match id {
        'b' => '\u{8}',
        'f' => '\u{c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'u' => hexescape(stream, 4, raw, error),
        'U' => hexescape(stream, 8, raw, error),
        '\\' => '\\',
        '"' => '"',
        _ => {
            stream.reset(&start);
            let offset = stream.offset_from(&raw.as_str());
            error.report_error(
                ParseError::new("missing escaped value")
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(EXPECTED_ESCAPES)
                    .with_unexpected(Span::new_unchecked(offset, offset)),
            );
            '\\'
        }
    }
}

fn hexescape(
    stream: &mut &str,
    num_digits: usize,
    raw: Raw<'_>,
    error: &mut dyn ErrorSink,
) -> char {
    let offset = stream
        .as_bytes()
        .offset_for(|b| !HEXDIG.contains_token(b))
        .unwrap_or_else(|| stream.eof_offset())
        .min(num_digits);
    #[cfg(feature = "unsafe")] // SAFETY: HEXDIG ensure `offset` is along UTF-8 boundary
    let value = unsafe { stream.next_slice_unchecked(offset) };
    #[cfg(not(feature = "unsafe"))]
    let value = stream.next_slice(offset);

    if value.len() != num_digits {
        let offset = stream.offset_from(&raw.as_str());
        error.report_error(
            ParseError::new("too few unicode value digits")
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Description("unicode hexadecimal value")])
                .with_unexpected(Span::new_unchecked(offset, offset)),
        );
        return '�';
    }

    let Some(value) = u32::from_str_radix(value, 16).ok().and_then(char::from_u32) else {
        let offset = value.offset_from(&raw.as_str());
        error.report_error(
            ParseError::new("invalid value")
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Description("unicode hexadecimal value")])
                .with_unexpected(Span::new_unchecked(offset, offset)),
        );
        return '�';
    };

    value
}

/// `HEXDIG = DIGIT / "A" / "B" / "C" / "D" / "E" / "F"`
const HEXDIG: (RangeInclusive<u8>, RangeInclusive<u8>, RangeInclusive<u8>) =
    (DIGIT, b'A'..=b'F', b'a'..=b'f');

/// `DIGIT = %x30-39 ; 0-9`
const DIGIT: RangeInclusive<u8> = b'0'..=b'9';

fn strip_start_newline(s: &str) -> &str {
    s.strip_prefix('\n')
        .or_else(|| s.strip_prefix("\r\n"))
        .unwrap_or(s)
}

/// Parse multi-line basic string
///
/// ```bnf
/// ;; Multiline Basic String
///
/// ml-basic-string = ml-basic-string-delim [ newline ] ml-basic-body
///                   ml-basic-string-delim
/// ml-basic-string-delim = 3quotation-mark
///
/// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
///
/// mlb-content = mlb-char / newline / mlb-escaped-nl
/// mlb-char = mlb-unescaped / escaped
/// mlb-quotes = 1*2quotation-mark
/// ```
pub(crate) fn decode_ml_basic_string<'i>(
    raw: Raw<'i>,
    output: &mut dyn StringBuilder<'i>,
    error: &mut dyn ErrorSink,
) {
    const INVALID_STRING: &str = "invalid multi-line basic string";

    let s = raw.as_str();
    let s = if let Some(stripped) = s.strip_prefix(ML_BASIC_STRING_DELIM) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("\"")])
                .with_unexpected(Span::new_unchecked(0, 0)),
        );
        s
    };
    let s = strip_start_newline(s);
    let mut s = if let Some(stripped) = s.strip_suffix(ML_BASIC_STRING_DELIM) {
        stripped
    } else {
        error.report_error(
            ParseError::new(INVALID_STRING)
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("\"")])
                .with_unexpected(Span::new_unchecked(raw.len(), raw.len())),
        );
        s
    };

    let segment = mlb_unescaped(&mut s);
    if !output.push_str(segment) {
        error.report_error(
            ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(0, raw.len())),
        );
    }
    while !s.is_empty() {
        if s.starts_with("\\") {
            let _ = s.next_token();

            if s.as_bytes()
                .first()
                .map(|b| (WSCHAR, b'\r', b'\n').contains_token(b))
                .unwrap_or(false)
            {
                mlb_escaped_nl(&mut s, raw, error);
            } else {
                let c = escape_seq_char(&mut s, raw, error);
                if !output.push_char(c) {
                    error.report_error(
                        ParseError::new(ALLOCATION_ERROR)
                            .with_unexpected(Span::new_unchecked(0, raw.len())),
                    );
                }
            }
        } else if s.starts_with("\r") {
            let offset = if s.starts_with("\r\n") {
                "\r\n".len()
            } else {
                let start = s.offset_from(&raw.as_str()) + 1;
                error.report_error(
                    ParseError::new("carriage return must be followed by newline")
                        .with_context(Span::new_unchecked(0, raw.len()))
                        .with_expected(&[Expected::Literal("\n")])
                        .with_unexpected(Span::new_unchecked(start, start)),
                );
                "\r".len()
            };
            #[cfg(feature = "unsafe")]
            // SAFETY: Newlines ensure `offset` is along UTF-8 boundary
            let newline = unsafe { s.next_slice_unchecked(offset) };
            #[cfg(not(feature = "unsafe"))]
            let newline = s.next_slice(offset);
            if !output.push_str(newline) {
                let start = newline.offset_from(&raw.as_str());
                let end = start + newline.len();
                error.report_error(
                    ParseError::new(ALLOCATION_ERROR)
                        .with_unexpected(Span::new_unchecked(start, end)),
                );
            }
        } else {
            let invalid = mlb_invalid(&mut s);
            let start = invalid.offset_from(&raw.as_str());
            let end = start + invalid.len();
            error.report_error(
                ParseError::new(INVALID_STRING)
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[Expected::Literal("\\"), Expected::Description("characters")])
                    .with_unexpected(Span::new_unchecked(start, end)),
            );
            let _ = output.push_str(invalid);
        }

        let segment = mlb_unescaped(&mut s);
        if !output.push_str(segment) {
            let start = segment.offset_from(&raw.as_str());
            let end = start + segment.len();
            error.report_error(
                ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(start, end)),
            );
        }
    }
}

/// ```bnf
/// mlb-escaped-nl = escape ws newline *( wschar / newline )
/// ```
fn mlb_escaped_nl(stream: &mut &str, raw: Raw<'_>, error: &mut dyn ErrorSink) {
    const INVALID_STRING: &str = "invalid multi-line basic string";
    let ws_offset = stream
        .as_bytes()
        .offset_for(|b| !WSCHAR.contains_token(b))
        .unwrap_or(stream.len());
    #[cfg(feature = "unsafe")] // SAFETY: WSCHAR ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(ws_offset);
    }
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(ws_offset);

    let start = stream.checkpoint();
    match stream.next_token() {
        Some('\n') => {}
        Some('\r') => {
            if stream.as_bytes().first() == Some(&b'\n') {
                let _ = stream.next_token();
            } else {
                let start = stream.offset_from(&raw.as_str());
                let end = start;
                error.report_error(
                    ParseError::new("carriage return must be followed by newline")
                        .with_context(Span::new_unchecked(0, raw.len()))
                        .with_expected(&[Expected::Literal("\n")])
                        .with_unexpected(Span::new_unchecked(start, end)),
                );
            }
        }
        _ => {
            stream.reset(&start);

            let start = stream.offset_from(&raw.as_str());
            let end = start;
            error.report_error(
                ParseError::new(INVALID_STRING)
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[Expected::Literal("\n")])
                    .with_unexpected(Span::new_unchecked(start, end)),
            );
        }
    }

    loop {
        let start_offset = stream.offset_from(&raw.as_str());

        let offset = stream
            .as_bytes()
            .offset_for(|b| !(WSCHAR, b'\n').contains_token(b))
            .unwrap_or(stream.len());
        #[cfg(feature = "unsafe")] // SAFETY: WSCHAR ensure `offset` is along UTF-8 boundary
        unsafe {
            stream.next_slice_unchecked(offset);
        }
        #[cfg(not(feature = "unsafe"))]
        stream.next_slice(offset);

        if stream.starts_with("\r") {
            let offset = if stream.starts_with("\r\n") {
                "\r\n".len()
            } else {
                let start = stream.offset_from(&raw.as_str()) + 1;
                error.report_error(
                    ParseError::new("carriage return must be followed by newline")
                        .with_context(Span::new_unchecked(0, raw.len()))
                        .with_expected(&[Expected::Literal("\n")])
                        .with_unexpected(Span::new_unchecked(start, start)),
                );
                "\r".len()
            };
            #[cfg(feature = "unsafe")]
            // SAFETY: Newlines ensure `offset` is along UTF-8 boundary
            let _ = unsafe { stream.next_slice_unchecked(offset) };
            #[cfg(not(feature = "unsafe"))]
            let _ = stream.next_slice(offset);
        }

        let end_offset = stream.offset_from(&raw.as_str());
        if start_offset == end_offset {
            break;
        }
    }
}

/// `mlb-unescaped` extended with `mlb-quotes` and `LF`
///
/// **warning:** `newline` is not validated
///
/// ```bnf
/// ml-basic-body = *mlb-content *( mlb-quotes 1*mlb-content ) [ mlb-quotes ]
///
/// mlb-content = mlb-char / newline / mlb-escaped-nl
/// mlb-char = mlb-unescaped / escaped
/// mlb-quotes = 1*2quotation-mark
/// mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii
/// ```
fn mlb_unescaped<'i>(stream: &mut &'i str) -> &'i str {
    let offset = stream
        .as_bytes()
        .offset_for(|b| !(MLB_UNESCAPED, b'"', b'\n').contains_token(b))
        .unwrap_or(stream.len());
    #[cfg(feature = "unsafe")] // SAFETY: BASIC_UNESCAPED ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    }
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset)
}

fn mlb_invalid<'i>(stream: &mut &'i str) -> &'i str {
    let offset = stream
        .as_bytes()
        .offset_for(|b| (MLB_UNESCAPED, b'"', b'\n', ESCAPE, '\r').contains_token(b))
        .unwrap_or(stream.len());
    #[cfg(feature = "unsafe")] // SAFETY: BASIC_UNESCAPED ensure `offset` is along UTF-8 boundary
    unsafe {
        stream.next_slice_unchecked(offset)
    }
    #[cfg(not(feature = "unsafe"))]
    stream.next_slice(offset)
}

/// `mlb-unescaped = wschar / %x21 / %x23-5B / %x5D-7E / non-ascii`
#[allow(clippy::type_complexity)]
const MLB_UNESCAPED: (
    (u8, u8),
    u8,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
) = (WSCHAR, 0x21, 0x23..=0x5B, 0x5D..=0x7E, NON_ASCII);

/// Parse unquoted key
///
/// ```bnf
/// unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _
/// ```
pub(crate) fn decode_unquoted_key<'i>(
    raw: Raw<'i>,
    output: &mut dyn StringBuilder<'i>,
    error: &mut dyn ErrorSink,
) {
    let s = raw.as_str();

    if s.is_empty() {
        error.report_error(
            ParseError::new("unquoted keys cannot be empty")
                .with_context(Span::new_unchecked(0, s.len()))
                .with_expected(&[
                    Expected::Description("letters"),
                    Expected::Description("numbers"),
                    Expected::Literal("-"),
                    Expected::Literal("_"),
                ])
                .with_unexpected(Span::new_unchecked(0, s.len())),
        );
    }

    for (i, b) in s.as_bytes().iter().enumerate() {
        if !UNQUOTED_CHAR.contains_token(b) {
            error.report_error(
                ParseError::new("invalid unquoted key")
                    .with_context(Span::new_unchecked(0, s.len()))
                    .with_expected(&[
                        Expected::Description("letters"),
                        Expected::Description("numbers"),
                        Expected::Literal("-"),
                        Expected::Literal("_"),
                    ])
                    .with_unexpected(Span::new_unchecked(i, i)),
            );
        }
    }

    if !output.push_str(s) {
        error.report_error(
            ParseError::new(ALLOCATION_ERROR).with_unexpected(Span::new_unchecked(0, raw.len())),
        );
    }
}

/// `unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _`
const UNQUOTED_CHAR: (
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    RangeInclusive<u8>,
    u8,
    u8,
) = (b'A'..=b'Z', b'a'..=b'z', b'0'..=b'9', b'-', b'_');

#[cfg(test)]
#[cfg(feature = "std")]
mod test {
    use super::*;
    use crate::decoder::Encoding;

    use alloc::borrow::Cow;

    use snapbox::assert_data_eq;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn literal_string() {
        let cases = [
            (
                r"'C:\Users\nodejs\templates'",
                str![[r#"C:\Users\nodejs\templates"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r"'\\ServerX\admin$\system32\'",
                str![[r#"\\ServerX\admin$\system32\"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"'Tom "Dubs" Preston-Werner'"#,
                str![[r#"Tom "Dubs" Preston-Werner"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r"'<\i\c*\s*>'",
                str![[r#"<\i\c*\s*>"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let mut actual = Cow::Borrowed("");
            decode_literal_string(
                Raw::new_unchecked(input, Some(Encoding::LiteralString), Default::default()),
                &mut actual,
                &mut error,
            );
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn ml_literal_string() {
        let cases = [
            (
                r"'''I [dw]on't need \d{2} apples'''",
                str![[r#"I [dw]on't need \d{2} apples"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"''''one_quote''''"#,
                str!["'one_quote'"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"'''
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.
'''"#,
                str![[r#"
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.

"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let mut actual = Cow::Borrowed("");
            decode_ml_literal_string(
                Raw::new_unchecked(input, Some(Encoding::MlLiteralString), Default::default()),
                &mut actual,
                &mut error,
            );
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn basic_string() {
        let cases = [
            (
                r#""""#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""content\"trailing""#,
                str![[r#"content"trailing"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""content\""#,
                str![[r#"content\"#]].raw(),
                str![[r#"
[
    ParseError {
        context: Some(
            0..10,
        ),
        description: "missing escaped value",
        expected: Some(
            [
                Literal(
                    "b",
                ),
                Literal(
                    "f",
                ),
                Literal(
                    "n",
                ),
                Literal(
                    "r",
                ),
                Literal(
                    "\\",
                ),
                Literal(
                    "\"",
                ),
                Literal(
                    "u",
                ),
                Literal(
                    "U",
                ),
            ],
        ),
        unexpected: Some(
            9..9,
        ),
    },
]

"#]]
                .raw(),
            ),
            (
                r#""content
trailing""#,
                str![[r#"
content
trailing
"#]]
                .raw(),
                str![[r#"
[
    ParseError {
        context: Some(
            0..18,
        ),
        description: "invalid basic string",
        expected: Some(
            [
                Description(
                    "non-double-quote visible characters",
                ),
                Literal(
                    "\\",
                ),
            ],
        ),
        unexpected: Some(
            8..9,
        ),
    },
]

"#]]
                .raw(),
            ),
            (
                r#""I'm a string. \"You can quote me\". Name\tJos\u00E9\nLocation\tSF. \U0002070E""#,
                str![[r#"
I'm a string. "You can quote me". Name	José
Location	SF. 𠜎
"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let mut actual = Cow::Borrowed("");
            decode_basic_string(
                Raw::new_unchecked(input, Some(Encoding::BasicString), Default::default()),
                &mut actual,
                &mut error,
            );
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn ml_basic_string() {
        let cases = [
            (
                r#""""
Roses are red
Violets are blue""""#,
                str![[r#"
Roses are red
Violets are blue
"#]]
                .raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"""" \""" """"#,
                str![[r#" """ "#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#"""" \\""""#,
                str![[r#" \"#]].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""
The quick brown \


  fox jumps over \
    the lazy dog.""""#,
                str!["The quick brown fox jumps over the lazy dog."].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""\
       The quick brown \
       fox jumps over \
       the lazy dog.\
       """"#,
                str!["The quick brown fox jumps over the lazy dog."].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""\
       """"#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""
\
  \
""""#,
                str![""].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                r#""""  """#,
                str![[r#"  """#]].raw(),
                str![[r#"
[
    ParseError {
        context: Some(
            0..7,
        ),
        description: "invalid multi-line basic string",
        expected: Some(
            [
                Literal(
                    "\"",
                ),
            ],
        ),
        unexpected: Some(
            7..7,
        ),
    },
]

"#]]
                .raw(),
            ),
            (
                r#""""  \""""#,
                str![[r#"  \"#]].raw(),
                str![[r#"
[
    ParseError {
        context: Some(
            0..9,
        ),
        description: "missing escaped value",
        expected: Some(
            [
                Literal(
                    "b",
                ),
                Literal(
                    "f",
                ),
                Literal(
                    "n",
                ),
                Literal(
                    "r",
                ),
                Literal(
                    "\\",
                ),
                Literal(
                    "\"",
                ),
                Literal(
                    "u",
                ),
                Literal(
                    "U",
                ),
            ],
        ),
        unexpected: Some(
            6..6,
        ),
    },
]

"#]]
                .raw(),
            ),
        ];
        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let mut actual = Cow::Borrowed("");
            decode_ml_basic_string(
                Raw::new_unchecked(input, Some(Encoding::MlBasicString), Default::default()),
                &mut actual,
                &mut error,
            );
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }

    #[test]
    fn unquoted_keys() {
        let cases = [
            (
                "a",
                str!["a"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "hello",
                str!["hello"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "-",
                str!["-"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "_",
                str!["_"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "-hello-world-",
                str!["-hello-world-"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "_hello_world_",
                str!["_hello_world_"].raw(),
                str![[r#"
[]

"#]]
                .raw(),
            ),
            (
                "",
                str![""].raw(),
                str![[r#"
[
    ParseError {
        context: Some(
            0..0,
        ),
        description: "unquoted keys cannot be empty",
        expected: Some(
            [
                Description(
                    "letters",
                ),
                Description(
                    "numbers",
                ),
                Literal(
                    "-",
                ),
                Literal(
                    "_",
                ),
            ],
        ),
        unexpected: Some(
            0..0,
        ),
    },
]

"#]]
                .raw(),
            ),
        ];

        for (input, expected, expected_error) in cases {
            let mut error = Vec::new();
            let mut actual = Cow::Borrowed("");
            decode_unquoted_key(
                Raw::new_unchecked(input, None, Default::default()),
                &mut actual,
                &mut error,
            );
            assert_data_eq!(actual.as_ref(), expected);
            assert_data_eq!(error.to_debug(), expected_error);
        }
    }
}
