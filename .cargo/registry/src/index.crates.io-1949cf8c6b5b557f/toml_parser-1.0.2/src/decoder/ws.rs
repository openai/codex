use core::ops::RangeInclusive;

use winnow::stream::ContainsToken as _;

use crate::lexer::COMMENT_START_SYMBOL;
use crate::ErrorSink;
use crate::Expected;
use crate::ParseError;
use crate::Raw;
use crate::Span;

/// Parse comment
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
pub(crate) fn decode_comment(raw: Raw<'_>, error: &mut dyn ErrorSink) {
    let s = raw.as_bytes();

    if s.first() != Some(&COMMENT_START_SYMBOL) {
        error.report_error(
            ParseError::new("missing comment start")
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("#")])
                .with_unexpected(Span::new_unchecked(0, 0)),
        );
    }

    for (i, b) in s.iter().copied().enumerate() {
        if !NON_EOL.contains_token(b) {
            error.report_error(
                ParseError::new("invalid comment character")
                    .with_context(Span::new_unchecked(0, raw.len()))
                    .with_expected(&[Expected::Description("printable characters")])
                    .with_unexpected(Span::new_unchecked(i, i)),
            );
        }
    }
}

// non-ascii = %x80-D7FF / %xE000-10FFFF
// - ASCII is 0xxxxxxx
// - First byte for UTF-8 is 11xxxxxx
// - Subsequent UTF-8 bytes are 10xxxxxx
pub(crate) const NON_ASCII: RangeInclusive<u8> = 0x80..=0xff;

// non-eol = %x09 / %x20-7E / non-ascii
pub(crate) const NON_EOL: (u8, RangeInclusive<u8>, RangeInclusive<u8>) =
    (0x09, 0x20..=0x7E, NON_ASCII);

/// Parse newline
///
/// ```bnf
///;; Newline
///
/// newline =  %x0A     ; LF
/// newline =/ %x0D.0A  ; CRLF
/// ```
pub(crate) fn decode_newline(raw: Raw<'_>, error: &mut dyn ErrorSink) {
    let s = raw.as_str();

    if s == "\r" {
        error.report_error(
            ParseError::new("carriage return must be followed by newline")
                .with_context(Span::new_unchecked(0, raw.len()))
                .with_expected(&[Expected::Literal("\n")])
                .with_unexpected(Span::new_unchecked(raw.len(), raw.len())),
        );
    }
}
