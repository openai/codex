//! Word-wrapping with URL-aware heuristics.
//!
//! The TUI renders text that frequently contains URLs — command output,
//! markdown, agent messages, tool-call results. Standard `textwrap`
//! hyphenation treats `/` and `-` as split points, which breaks URLs
//! across lines and makes them unclickable in terminal emulators.
//!
//! This module provides two wrapping paths:
//!
//! - **Standard** (`word_wrap_line`, `word_wrap_lines`): delegates to
//!   `textwrap` with the caller's options unchanged. Used when the
//!   content is known to be plain prose.
//! - **Adaptive** (`adaptive_wrap_line`, `adaptive_wrap_lines`):
//!   inspects the line for URL-like tokens; if any are found, the
//!   wrapping switches to `AsciiSpace` word separation and a custom
//!   `WordSplitter` that refuses to split URL tokens. Non-URL tokens
//!   on the same line still break at every character boundary (the
//!   custom splitter returns all char indices for non-URL words).
//!
//! Callers that *might* encounter URLs should use the `adaptive_*`
//! functions. Callers that definitely will not (code blocks, pure
//! numeric output) can use the standard path for speed.
//!
//! URL detection is heuristic — see [`text_contains_url_like`] for the
//! rules. False positives suppress hyphenation for that line; false
//! negatives let a URL get split. The heuristic is intentionally
//! conservative: file paths like `src/main.rs` are not matched.

use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::ops::Range;
use textwrap::Options;

use crate::render::line_utils::push_owned_lines;

/// Returns byte-ranges into `text` for each wrapped line, including
/// trailing whitespace and a +1 sentinel byte. Used by the textarea
/// cursor-position logic.
pub(crate) fn wrap_ranges<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut cursor = 0usize;
    for (line_index, line) in textwrap::wrap(text, &opts).iter().enumerate() {
        match line {
            std::borrow::Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                let trailing_spaces = text[end..].chars().take_while(|c| *c == ' ').count();
                lines.push(start..end + trailing_spaces + 1);
                cursor = end + trailing_spaces;
            }
            std::borrow::Cow::Owned(slice) => {
                let synthetic_prefix = if line_index == 0 {
                    opts.initial_indent
                } else {
                    opts.subsequent_indent
                };
                let mapped = map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix);
                let trailing_spaces = text[mapped.end..].chars().take_while(|c| *c == ' ').count();
                lines.push(mapped.start..mapped.end + trailing_spaces + 1);
                cursor = mapped.end + trailing_spaces;
            }
        }
    }
    lines
}

/// Like `wrap_ranges` but returns ranges without trailing whitespace and
/// without the sentinel extra byte. Suitable for general wrapping where
/// trailing spaces should not be preserved.
pub(crate) fn wrap_ranges_trim<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut cursor = 0usize;
    for (line_index, line) in textwrap::wrap(text, &opts).iter().enumerate() {
        match line {
            std::borrow::Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                lines.push(start..end);
                cursor = end;
            }
            std::borrow::Cow::Owned(slice) => {
                let synthetic_prefix = if line_index == 0 {
                    opts.initial_indent
                } else {
                    opts.subsequent_indent
                };
                let mapped = map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix);
                lines.push(mapped.clone());
                cursor = mapped.end;
            }
        }
    }
    lines
}

/// Maps an owned (materialized) wrapped line back to a byte range in `text`.
///
/// `textwrap` returns `Cow::Owned` when it inserts a hyphenation penalty
/// character (typically `-`) that does not exist in the source. This
/// function walks the owned string character-by-character against the
/// source, skipping trailing penalty chars, and returns the
/// corresponding source byte range starting from `cursor`.
fn map_owned_wrapped_line_to_range(
    text: &str,
    cursor: usize,
    wrapped: &str,
    synthetic_prefix: &str,
) -> Range<usize> {
    let wrapped = if synthetic_prefix.is_empty() {
        wrapped
    } else {
        wrapped.strip_prefix(synthetic_prefix).unwrap_or(wrapped)
    };

    let mut start = cursor;
    while start < text.len() && !wrapped.starts_with(' ') {
        let Some(ch) = text[start..].chars().next() else {
            break;
        };
        if ch != ' ' {
            break;
        }
        start += ch.len_utf8();
    }

    let mut end = start;
    let mut saw_source_char = false;
    let mut chars = wrapped.chars().peekable();
    while let Some(ch) = chars.next() {
        if end < text.len() {
            let Some(src) = text[end..].chars().next() else {
                unreachable!("checked end < text.len()");
            };
            if ch == src {
                end += src.len_utf8();
                saw_source_char = true;
                continue;
            }
        }

        // textwrap can materialize owned lines when penalties are inserted.
        // The default penalty is a trailing '-'; it does not correspond to
        // source bytes, so we skip it while keeping byte ranges in source text.
        if ch == '-' && chars.peek().is_none() {
            continue;
        }

        // Non-source chars can be synthesized by textwrap in owned output
        // (e.g. non-space indent prefixes). Keep going and map the source bytes
        // we can confidently match instead of crashing the app.
        if !saw_source_char {
            continue;
        }

        tracing::warn!(
            wrapped = %wrapped,
            cursor,
            end,
            "wrap_ranges: could not fully map owned line; returning partial source range"
        );
        break;
    }

    start..end
}

/// Returns `true` if any whitespace-delimited token in `line` looks like a URL.
///
/// Concatenates all span contents and delegates to [`text_contains_url_like`].
pub(crate) fn line_contains_url_like(line: &Line<'_>) -> bool {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text_contains_url_like(&text)
}

/// Returns `true` if `line` contains both a URL-like token and at least one
/// substantive non-URL token.
///
/// Decorative marker tokens (for example list prefixes like `-`, `1.`, `|`,
/// `│`) are ignored for the non-URL side of this check.
pub(crate) fn line_has_mixed_url_and_non_url_tokens(line: &Line<'_>) -> bool {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text_has_mixed_url_and_non_url_tokens(&text)
}

/// Returns `true` if any whitespace-delimited token in `text` looks like a URL.
///
/// Recognized patterns:
/// - Absolute URLs with a scheme (`https://…`, `ftp://…`, custom `myapp://…`).
/// - Bare domain URLs (`example.com/path`, `www.example.com`, `localhost:3000/api`).
/// - IPv4 hosts with a path (`192.168.1.1:8080/health`).
///
/// Surrounding punctuation (`()[]{}< >,.;:!'"`) is stripped before
/// checking. Tokens that look like file paths (`src/main.rs`, `foo/bar`)
/// are intentionally rejected — the host portion must be a valid domain
/// name (with a recognized TLD), an IPv4 address, or `localhost`.
pub(crate) fn text_contains_url_like(text: &str) -> bool {
    text.split_ascii_whitespace().any(is_url_like_token)
}

/// Returns `true` if `text` contains at least one URL-like token and at least
/// one substantive non-URL token.
fn text_has_mixed_url_and_non_url_tokens(text: &str) -> bool {
    let mut saw_url = false;
    let mut saw_non_url = false;

    for raw_token in text.split_ascii_whitespace() {
        if is_url_like_token(raw_token) {
            saw_url = true;
        } else if is_substantive_non_url_token(raw_token) {
            saw_non_url = true;
        }

        if saw_url && saw_non_url {
            return true;
        }
    }

    false
}

/// Decides whether a single whitespace-delimited token is URL-like.
///
/// Strips surrounding punctuation, then checks for an absolute URL
/// (with `://`) or a bare domain URL (recognized host + path/query/fragment).
fn is_url_like_token(raw_token: &str) -> bool {
    let token = trim_url_token(raw_token);
    !token.is_empty() && (is_absolute_url_like(token) || is_bare_url_like(token))
}

fn is_substantive_non_url_token(raw_token: &str) -> bool {
    let token = trim_url_token(raw_token);
    if token.is_empty() || is_decorative_marker_token(raw_token, token) {
        return false;
    }

    token.chars().any(char::is_alphanumeric)
}

fn is_decorative_marker_token(raw_token: &str, token: &str) -> bool {
    let raw = raw_token.trim();
    matches!(
        raw,
        "-" | "*"
            | "+"
            | "•"
            | "◦"
            | "▪"
            | ">"
            | "|"
            | "│"
            | "┆"
            | "└"
            | "├"
            | "┌"
            | "┐"
            | "┘"
            | "┼"
    ) || is_ordered_list_marker(raw, token)
}

fn is_ordered_list_marker(raw_token: &str, token: &str) -> bool {
    token.chars().all(|c| c.is_ascii_digit())
        && (raw_token.ends_with('.') || raw_token.ends_with(')'))
}

fn trim_url_token(token: &str) -> &str {
    token.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | ','
                | '.'
                | ';'
                | ':'
                | '!'
                | '\''
                | '"'
        )
    })
}

/// Checks for `scheme://host` patterns. Uses `url::Url::parse` for
/// well-known schemes; falls back to `has_valid_scheme_prefix` for
/// custom schemes that the `url` crate rejects.
fn is_absolute_url_like(token: &str) -> bool {
    if !token.contains("://") {
        return false;
    }

    if let Ok(url) = url::Url::parse(token) {
        let scheme = url.scheme().to_ascii_lowercase();
        if matches!(
            scheme.as_str(),
            "http" | "https" | "ftp" | "ftps" | "ws" | "wss"
        ) {
            return url.host_str().is_some();
        }
        return true;
    }

    has_valid_scheme_prefix(token)
}

fn has_valid_scheme_prefix(token: &str) -> bool {
    let Some((scheme, rest)) = token.split_once("://") else {
        return false;
    };
    if scheme.is_empty() || rest.is_empty() {
        return false;
    }

    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

/// Checks for bare-domain URLs without a scheme: `host[:port]/path`,
/// `host[:port]?query`, or `host[:port]#fragment`.
///
/// Requires that the host is `localhost`, an IPv4 address, or a valid
/// domain name. Bare `host.tld` without a path/query/fragment is only
/// accepted when the host starts with `www.`.
///
/// IPv6 bracket notation (`[::1]:8080`) is intentionally not handled.
fn is_bare_url_like(token: &str) -> bool {
    let (host_port, has_trailer) = split_host_port_and_trailer(token);
    if host_port.is_empty() {
        return false;
    }

    // Require URL-ish trailer for bare hosts unless token starts with www.
    if !has_trailer && !host_port.to_ascii_lowercase().starts_with("www.") {
        return false;
    }

    let (host, port) = split_host_and_port(host_port);
    if host.is_empty() {
        return false;
    }
    if let Some(port) = port
        && !is_valid_port(port)
    {
        return false;
    }

    host.eq_ignore_ascii_case("localhost") || is_ipv4(host) || is_domain_name(host)
}

fn split_host_port_and_trailer(token: &str) -> (&str, bool) {
    if let Some(idx) = token.find(['/', '?', '#']) {
        (&token[..idx], true)
    } else {
        (token, false)
    }
}

fn split_host_and_port(host_port: &str) -> (&str, Option<&str>) {
    // We intentionally do not treat bracketed IPv6 as URL-like in this first pass.
    if host_port.starts_with('[') {
        return (host_port, None);
    }

    if let Some((host, port)) = host_port.rsplit_once(':')
        && !host.is_empty()
        && !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return (host, Some(port));
    }

    (host_port, None)
}

fn is_valid_port(port: &str) -> bool {
    if port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    port.parse::<u16>().is_ok()
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    parts
        .iter()
        .all(|part| !part.is_empty() && part.parse::<u8>().is_ok())
}

fn is_domain_name(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if !host.contains('.') {
        return false;
    }

    let mut labels = host.split('.');
    let Some(tld) = labels.next_back() else {
        return false;
    };
    if !is_tld(tld) {
        return false;
    }

    labels.all(is_domain_label)
}

fn is_tld(label: &str) -> bool {
    (2..=63).contains(&label.len()) && label.chars().all(|c| c.is_ascii_alphabetic())
}

fn is_domain_label(label: &str) -> bool {
    if label.is_empty() || label.len() > 63 {
        return false;
    }

    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(last) = label.chars().next_back() else {
        return false;
    };

    first.is_ascii_alphanumeric()
        && last.is_ascii_alphanumeric()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Reconfigures wrapping options so that URL-like tokens are never split.
///
/// Sets `AsciiSpace` word separation (so `/` and `-` inside URLs are
/// not treated as break points), disables `break_words`, and installs a
/// custom `WordSplitter` that returns no split points for URL tokens
/// while still allowing character-level splitting for non-URL words.
pub(crate) fn url_preserving_wrap_options<'a>(opts: RtOptions<'a>) -> RtOptions<'a> {
    opts.word_separator(textwrap::WordSeparator::AsciiSpace)
        .word_splitter(textwrap::WordSplitter::Custom(split_non_url_word))
        .break_words(false)
}

/// Custom `textwrap::WordSplitter` callback. Returns empty (no split
/// points) for URL-like tokens so they are kept intact; returns every
/// char-boundary index for everything else so non-URL words can still
/// break at any position.
fn split_non_url_word(word: &str) -> Vec<usize> {
    if is_url_like_token(word) {
        return Vec::new();
    }

    word.char_indices().skip(1).map(|(idx, _)| idx).collect()
}

/// Wraps a single ratatui `Line`, automatically switching to
/// URL-preserving options when the line contains a URL-like token.
///
/// When no URL is detected, wrapping behavior is identical to
/// [`word_wrap_line`]. When a URL is detected, the line is wrapped with
/// [`url_preserving_wrap_options`] — URLs stay intact while non-URL
/// words on the same line still break normally.
#[must_use]
pub(crate) fn adaptive_wrap_line<'a>(line: &'a Line<'a>, base: RtOptions<'a>) -> Vec<Line<'a>> {
    let selected = if line_contains_url_like(line) {
        url_preserving_wrap_options(base)
    } else {
        base
    };
    word_wrap_line(line, selected)
}

/// Wraps multiple input lines with URL-aware heuristics, applying
/// `initial_indent` to the first line and `subsequent_indent` to the
/// rest. Each line is independently checked for URLs; URL detection on
/// one line does not affect wrapping of the others.
///
/// This is the multi-line counterpart to [`adaptive_wrap_line`] and is
/// the primary wrapping entry point for most history-cell rendering.
#[allow(private_bounds)]
pub(crate) fn adaptive_wrap_lines<'a, I, L>(
    lines: I,
    width_or_options: RtOptions<'a>,
) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
{
    let base_opts = width_or_options;
    let mut out: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            base_opts
                .clone()
                .initial_indent(base_opts.subsequent_indent.clone())
        };

        let wrapped = adaptive_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }

    out
}

#[derive(Debug, Clone)]
pub struct RtOptions<'a> {
    /// The width in columns at which the text will be wrapped.
    pub width: usize,
    /// Line ending used for breaking lines.
    pub line_ending: textwrap::LineEnding,
    /// Indentation used for the first line of output. See the
    /// [`Options::initial_indent`] method.
    pub initial_indent: Line<'a>,
    /// Indentation used for subsequent lines of output. See the
    /// [`Options::subsequent_indent`] method.
    pub subsequent_indent: Line<'a>,
    /// Allow long words to be broken if they cannot fit on a line.
    /// When set to `false`, some lines may be longer than
    /// `self.width`. See the [`Options::break_words`] method.
    pub break_words: bool,
    /// Wrapping algorithm to use, see the implementations of the
    /// [`WrapAlgorithm`] trait for details.
    pub wrap_algorithm: textwrap::WrapAlgorithm,
    /// The line breaking algorithm to use, see the [`WordSeparator`]
    /// trait for an overview and possible implementations.
    pub word_separator: textwrap::WordSeparator,
    /// The method for splitting words. This can be used to prohibit
    /// splitting words on hyphens, or it can be used to implement
    /// language-aware machine hyphenation.
    pub word_splitter: textwrap::WordSplitter,
}
impl From<usize> for RtOptions<'_> {
    fn from(width: usize) -> Self {
        RtOptions::new(width)
    }
}

#[allow(dead_code)]
impl<'a> RtOptions<'a> {
    pub fn new(width: usize) -> Self {
        RtOptions {
            width,
            line_ending: textwrap::LineEnding::LF,
            initial_indent: Line::default(),
            subsequent_indent: Line::default(),
            break_words: true,
            word_separator: textwrap::WordSeparator::new(),
            wrap_algorithm: textwrap::WrapAlgorithm::FirstFit,
            word_splitter: textwrap::WordSplitter::HyphenSplitter,
        }
    }

    pub fn line_ending(self, line_ending: textwrap::LineEnding) -> Self {
        RtOptions {
            line_ending,
            ..self
        }
    }

    pub fn width(self, width: usize) -> Self {
        RtOptions { width, ..self }
    }

    pub fn initial_indent(self, initial_indent: Line<'a>) -> Self {
        RtOptions {
            initial_indent,
            ..self
        }
    }

    pub fn subsequent_indent(self, subsequent_indent: Line<'a>) -> Self {
        RtOptions {
            subsequent_indent,
            ..self
        }
    }

    pub fn break_words(self, break_words: bool) -> Self {
        RtOptions {
            break_words,
            ..self
        }
    }

    pub fn word_separator(self, word_separator: textwrap::WordSeparator) -> RtOptions<'a> {
        RtOptions {
            word_separator,
            ..self
        }
    }

    pub fn wrap_algorithm(self, wrap_algorithm: textwrap::WrapAlgorithm) -> RtOptions<'a> {
        RtOptions {
            wrap_algorithm,
            ..self
        }
    }

    pub fn word_splitter(self, word_splitter: textwrap::WordSplitter) -> RtOptions<'a> {
        RtOptions {
            word_splitter,
            ..self
        }
    }
}

#[must_use]
pub(crate) fn word_wrap_line<'a, O>(line: &'a Line<'a>, width_or_options: O) -> Vec<Line<'a>>
where
    O: Into<RtOptions<'a>>,
{
    // Flatten the line and record span byte ranges.
    let mut flat = String::new();
    let mut span_bounds = Vec::new();
    let mut acc = 0usize;
    for s in &line.spans {
        let text = s.content.as_ref();
        let start = acc;
        flat.push_str(text);
        acc += text.len();
        span_bounds.push((start..acc, s.style));
    }

    let rt_opts: RtOptions<'a> = width_or_options.into();
    let opts = Options::new(rt_opts.width)
        .line_ending(rt_opts.line_ending)
        .break_words(rt_opts.break_words)
        .wrap_algorithm(rt_opts.wrap_algorithm)
        .word_separator(rt_opts.word_separator)
        .word_splitter(rt_opts.word_splitter);

    let mut out: Vec<Line<'a>> = Vec::new();

    // Compute first line range with reduced width due to initial indent.
    let initial_width_available = opts
        .width
        .saturating_sub(rt_opts.initial_indent.width())
        .max(1);
    let initial_wrapped = wrap_ranges_trim(&flat, opts.clone().width(initial_width_available));
    let Some(first_line_range) = initial_wrapped.first() else {
        return vec![rt_opts.initial_indent.clone()];
    };

    // Build first wrapped line with initial indent.
    let mut first_line = rt_opts.initial_indent.clone().style(line.style);
    {
        let sliced = slice_line_spans(line, &span_bounds, first_line_range);
        let mut spans = first_line.spans;
        spans.append(
            &mut sliced
                .spans
                .into_iter()
                .map(|s| s.patch_style(line.style))
                .collect(),
        );
        first_line.spans = spans;
        out.push(first_line);
    }

    // Wrap the remainder using subsequent indent width and map back to original indices.
    let base = first_line_range.end;
    let skip_leading_spaces = flat[base..].chars().take_while(|c| *c == ' ').count();
    let base = base + skip_leading_spaces;
    let subsequent_width_available = opts
        .width
        .saturating_sub(rt_opts.subsequent_indent.width())
        .max(1);
    let remaining_wrapped = wrap_ranges_trim(&flat[base..], opts.width(subsequent_width_available));
    for r in &remaining_wrapped {
        if r.is_empty() {
            continue;
        }
        let mut subsequent_line = rt_opts.subsequent_indent.clone().style(line.style);
        let offset_range = (r.start + base)..(r.end + base);
        let sliced = slice_line_spans(line, &span_bounds, &offset_range);
        let mut spans = subsequent_line.spans;
        spans.append(
            &mut sliced
                .spans
                .into_iter()
                .map(|s| s.patch_style(line.style))
                .collect(),
        );
        subsequent_line.spans = spans;
        out.push(subsequent_line);
    }

    out
}

/// Utilities to allow wrapping either borrowed or owned lines.
#[derive(Debug)]
enum LineInput<'a> {
    Borrowed(&'a Line<'a>),
    Owned(Line<'a>),
}

impl<'a> LineInput<'a> {
    fn as_ref(&self) -> &Line<'a> {
        match self {
            LineInput::Borrowed(line) => line,
            LineInput::Owned(line) => line,
        }
    }
}

/// This trait makes it easier to pass whatever we need into word_wrap_lines.
trait IntoLineInput<'a> {
    fn into_line_input(self) -> LineInput<'a>;
}

impl<'a> IntoLineInput<'a> for &'a Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for &'a mut Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(self)
    }
}

impl<'a> IntoLineInput<'a> for String {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for &'a str {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Cow<'a, str> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Span<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Vec<Span<'a>> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

/// Wrap a sequence of lines, applying the initial indent only to the very first
/// output line, and using the subsequent indent for all later wrapped pieces.
#[allow(private_bounds)] // IntoLineInput isn't public, but it doesn't really need to be.
pub(crate) fn word_wrap_lines<'a, I, O, L>(lines: I, width_or_options: O) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
    O: Into<RtOptions<'a>>,
{
    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            let mut o = base_opts.clone();
            let sub = o.subsequent_indent.clone();
            o = o.initial_indent(sub);
            o
        };
        let wrapped = word_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }

    out
}

#[allow(dead_code)]
pub(crate) fn word_wrap_lines_borrowed<'a, I, O>(lines: I, width_or_options: O) -> Vec<Line<'a>>
where
    I: IntoIterator<Item = &'a Line<'a>>,
    O: Into<RtOptions<'a>>,
{
    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'a>> = Vec::new();
    let mut first = true;
    for line in lines.into_iter() {
        let opts = if first {
            base_opts.clone()
        } else {
            base_opts
                .clone()
                .initial_indent(base_opts.subsequent_indent.clone())
        };
        out.extend(word_wrap_line(line, opts));
        first = false;
    }
    out
}

fn slice_line_spans<'a>(
    original: &'a Line<'a>,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    range: &Range<usize>,
) -> Line<'a> {
    let start_byte = range.start;
    let end_byte = range.end;
    let mut acc: Vec<Span<'a>> = Vec::new();
    for (i, (range, style)) in span_bounds.iter().enumerate() {
        let s = range.start;
        let e = range.end;
        if e <= start_byte {
            continue;
        }
        if s >= end_byte {
            break;
        }
        let seg_start = start_byte.max(s);
        let seg_end = end_byte.min(e);
        if seg_end > seg_start {
            let local_start = seg_start - s;
            let local_end = seg_end - s;
            let content = original.spans[i].content.as_ref();
            let slice = &content[local_start..local_end];
            acc.push(Span {
                style: *style,
                content: std::borrow::Cow::Borrowed(slice),
            });
        }
        if e >= end_byte {
            break;
        }
    }
    Line {
        style: original.style,
        alignment: original.alignment,
        spans: acc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools as _;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Stylize;
    use std::string::ToString;

    fn concat_line(line: &Line) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn trivial_unstyled_no_indents_wide_width() {
        let line = Line::from("hello");
        let out = word_wrap_line(&line, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "hello");
    }

    #[test]
    fn simple_unstyled_wrap_narrow_width() {
        let line = Line::from("hello world");
        let out = word_wrap_line(&line, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn simple_styled_wrap_preserves_styles() {
        let line = Line::from(vec!["hello ".red(), "world".into()]);
        let out = word_wrap_line(&line, 6);
        assert_eq!(out.len(), 2);
        // First line should carry the red style
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(out[0].spans.len(), 1);
        assert_eq!(out[0].spans[0].style.fg, Some(Color::Red));
        // Second line is unstyled
        assert_eq!(concat_line(&out[1]), "world");
        assert_eq!(out[1].spans.len(), 1);
        assert_eq!(out[1].spans[0].style.fg, None);
    }

    #[test]
    fn with_initial_and_subsequent_indents() {
        let opts = RtOptions::new(8)
            .initial_indent(Line::from("- "))
            .subsequent_indent(Line::from("  "));
        let line = Line::from("hello world foo");
        let out = word_wrap_line(&line, opts);
        // Expect three lines with proper prefixes
        assert!(concat_line(&out[0]).starts_with("- "));
        assert!(concat_line(&out[1]).starts_with("  "));
        assert!(concat_line(&out[2]).starts_with("  "));
        // And content roughly segmented
        assert_eq!(concat_line(&out[0]), "- hello");
        assert_eq!(concat_line(&out[1]), "  world");
        assert_eq!(concat_line(&out[2]), "  foo");
    }

    #[test]
    fn empty_initial_indent_subsequent_spaces() {
        let opts = RtOptions::new(8)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from("    "));
        let line = Line::from("hello world foobar");
        let out = word_wrap_line(&line, opts);
        assert!(concat_line(&out[0]).starts_with("hello"));
        for l in &out[1..] {
            assert!(concat_line(l).starts_with("    "));
        }
    }

    #[test]
    fn empty_input_yields_single_empty_line() {
        let line = Line::from("");
        let out = word_wrap_line(&line, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "");
    }

    #[test]
    fn leading_spaces_preserved_on_first_line() {
        let line = Line::from("   hello");
        let out = word_wrap_line(&line, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "   hello");
    }

    #[test]
    fn multiple_spaces_between_words_dont_start_next_line_with_spaces() {
        let line = Line::from("hello   world");
        let out = word_wrap_line(&line, 8);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn break_words_false_allows_overflow_for_long_word() {
        let opts = RtOptions::new(5).break_words(false);
        let line = Line::from("supercalifragilistic");
        let out = word_wrap_line(&line, opts);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "supercalifragilistic");
    }

    #[test]
    fn hyphen_splitter_breaks_at_hyphen() {
        let line = Line::from("hello-world");
        let out = word_wrap_line(&line, 7);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello-");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn indent_consumes_width_leaving_one_char_space() {
        let opts = RtOptions::new(4)
            .initial_indent(Line::from(">>>>"))
            .subsequent_indent(Line::from("--"));
        let line = Line::from("hello");
        let out = word_wrap_line(&line, opts);
        assert_eq!(out.len(), 3);
        assert_eq!(concat_line(&out[0]), ">>>>h");
        assert_eq!(concat_line(&out[1]), "--el");
        assert_eq!(concat_line(&out[2]), "--lo");
    }

    #[test]
    fn wide_unicode_wraps_by_display_width() {
        let line = Line::from("😀😀😀");
        let out = word_wrap_line(&line, 4);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "😀😀");
        assert_eq!(concat_line(&out[1]), "😀");
    }

    #[test]
    fn styled_split_within_span_preserves_style() {
        use ratatui::style::Stylize;
        let line = Line::from(vec!["abcd".red()]);
        let out = word_wrap_line(&line, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].spans.len(), 1);
        assert_eq!(out[1].spans.len(), 1);
        assert_eq!(out[0].spans[0].style.fg, Some(Color::Red));
        assert_eq!(out[1].spans[0].style.fg, Some(Color::Red));
        assert_eq!(concat_line(&out[0]), "ab");
        assert_eq!(concat_line(&out[1]), "cd");
    }

    #[test]
    fn wrap_lines_applies_initial_indent_only_once() {
        let opts = RtOptions::new(8)
            .initial_indent(Line::from("- "))
            .subsequent_indent(Line::from("  "));

        let lines = vec![Line::from("hello world"), Line::from("foo bar baz")];
        let out = word_wrap_lines(lines, opts);

        // Expect: first line prefixed with "- ", subsequent wrapped pieces with "  "
        // and for the second input line, there should be no "- " prefix on its first piece
        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert!(rendered[0].starts_with("- "));
        for r in rendered.iter().skip(1) {
            assert!(r.starts_with("  "));
        }
    }

    #[test]
    fn wrap_lines_without_indents_is_concat_of_single_wraps() {
        let lines = vec![Line::from("hello"), Line::from("world!")];
        let out = word_wrap_lines(lines, 10);
        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert_eq!(rendered, vec!["hello", "world!"]);
    }

    #[test]
    fn wrap_lines_borrowed_applies_initial_indent_only_once() {
        let opts = RtOptions::new(8)
            .initial_indent(Line::from("- "))
            .subsequent_indent(Line::from("  "));

        let lines = [Line::from("hello world"), Line::from("foo bar baz")];
        let out = word_wrap_lines_borrowed(lines.iter(), opts);

        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert!(rendered.first().unwrap().starts_with("- "));
        for r in rendered.iter().skip(1) {
            assert!(r.starts_with("  "));
        }
    }

    #[test]
    fn wrap_lines_borrowed_without_indents_is_concat_of_single_wraps() {
        let lines = [Line::from("hello"), Line::from("world!")];
        let out = word_wrap_lines_borrowed(lines.iter(), 10);
        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert_eq!(rendered, vec!["hello", "world!"]);
    }

    #[test]
    fn wrap_lines_accepts_borrowed_iterators() {
        let lines = [Line::from("hello world"), Line::from("foo bar baz")];
        let out = word_wrap_lines(lines, 10);
        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert_eq!(rendered, vec!["hello", "world", "foo bar", "baz"]);
    }

    #[test]
    fn wrap_lines_accepts_str_slices() {
        let lines = ["hello world", "goodnight moon"];
        let out = word_wrap_lines(lines, 12);
        let rendered: Vec<String> = out.iter().map(concat_line).collect();
        assert_eq!(rendered, vec!["hello world", "goodnight", "moon"]);
    }

    #[test]
    fn line_height_counts_double_width_emoji() {
        let line = "😀😀😀".into(); // each emoji ~ width 2
        assert_eq!(word_wrap_line(&line, 4).len(), 2);
        assert_eq!(word_wrap_line(&line, 2).len(), 3);
        assert_eq!(word_wrap_line(&line, 6).len(), 1);
    }

    #[test]
    fn word_wrap_does_not_split_words_simple_english() {
        let sample = "Years passed, and Willowmere thrived in peace and friendship. Mira’s herb garden flourished with both ordinary and enchanted plants, and travelers spoke of the kindness of the woman who tended them.";
        let line = Line::from(sample);
        let lines = [line];
        // Force small width to exercise wrapping at spaces.
        let wrapped = word_wrap_lines_borrowed(&lines, 40);
        let joined: String = wrapped.iter().map(ToString::to_string).join("\n");
        assert_eq!(
            joined,
            r#"Years passed, and Willowmere thrived in
peace and friendship. Mira’s herb garden
flourished with both ordinary and
enchanted plants, and travelers spoke of
the kindness of the woman who tended
them."#
        );
    }

    #[test]
    fn ascii_space_separator_with_no_hyphenation_keeps_url_intact() {
        let line = Line::from(
            "http://example.com/long-url-with-dashes-wider-than-terminal-window/blah-blah-blah-text/more-gibberish-text",
        );
        let opts = RtOptions::new(24)
            .word_separator(textwrap::WordSeparator::AsciiSpace)
            .word_splitter(textwrap::WordSplitter::NoHyphenation)
            .break_words(false);

        let out = word_wrap_line(&line, opts);

        assert_eq!(out.len(), 1);
        assert_eq!(
            concat_line(&out[0]),
            "http://example.com/long-url-with-dashes-wider-than-terminal-window/blah-blah-blah-text/more-gibberish-text"
        );
    }

    #[test]
    fn text_contains_url_like_matches_expected_tokens() {
        let positives = [
            "https://example.com/a/b",
            "ftp://host/path",
            "www.example.com/path?x=1",
            "example.test/path#frag",
            "localhost:3000/api",
            "127.0.0.1:8080/health",
            "(https://example.com/wrapped-in-parens)",
        ];

        for text in positives {
            assert!(
                text_contains_url_like(text),
                "expected URL-like match for {text:?}"
            );
        }
    }

    #[test]
    fn text_contains_url_like_rejects_non_urls() {
        let negatives = [
            "src/main.rs",
            "foo/bar",
            "key:value",
            "just-some-text-with-dashes",
            "hello.world", // no path/query/fragment and no www
        ];

        for text in negatives {
            assert!(
                !text_contains_url_like(text),
                "did not expect URL-like match for {text:?}"
            );
        }
    }

    #[test]
    fn line_contains_url_like_checks_across_spans() {
        let line = Line::from(vec![
            "see ".into(),
            "https://example.com/a/very/long/path".cyan(),
            " for details".into(),
        ]);

        assert!(line_contains_url_like(&line));
    }

    #[test]
    fn line_has_mixed_url_and_non_url_tokens_detects_prose_plus_url() {
        let line = Line::from("see https://example.com/path for details");
        assert!(line_has_mixed_url_and_non_url_tokens(&line));
    }

    #[test]
    fn line_has_mixed_url_and_non_url_tokens_ignores_pipe_prefix() {
        let line = Line::from(vec!["  │ ".into(), "https://example.com/path".into()]);
        assert!(!line_has_mixed_url_and_non_url_tokens(&line));
    }

    #[test]
    fn line_has_mixed_url_and_non_url_tokens_ignores_ordered_list_marker() {
        let line = Line::from("1. https://example.com/path");
        assert!(!line_has_mixed_url_and_non_url_tokens(&line));
    }

    #[test]
    fn text_contains_url_like_accepts_custom_scheme_with_separator() {
        assert!(text_contains_url_like("myapp://open/some/path"));
    }

    #[test]
    fn text_contains_url_like_rejects_invalid_ports() {
        assert!(!text_contains_url_like("localhost:99999/path"));
        assert!(!text_contains_url_like("example.com:abc/path"));
    }

    #[test]
    fn adaptive_wrap_line_keeps_long_url_like_token_intact() {
        let line = Line::from("example.test/a-very-long-path-with-many-segments-and-query?x=1&y=2");
        let out = adaptive_wrap_line(&line, RtOptions::new(20));
        assert_eq!(out.len(), 1);
        assert_eq!(
            concat_line(&out[0]),
            "example.test/a-very-long-path-with-many-segments-and-query?x=1&y=2"
        );
    }

    #[test]
    fn adaptive_wrap_line_preserves_default_behavior_for_non_url_tokens() {
        let line = Line::from("a_very_long_token_without_spaces_to_force_wrapping");
        let out = adaptive_wrap_line(&line, RtOptions::new(20));
        assert!(
            out.len() > 1,
            "expected non-url token to wrap with default options"
        );
    }

    #[test]
    fn adaptive_wrap_line_mixed_line_wraps_long_non_url_token() {
        let long_non_url = "a_very_long_token_without_spaces_to_force_wrapping";
        let line = Line::from(format!("see https://ex.com {long_non_url}"));
        let out = adaptive_wrap_line(&line, RtOptions::new(24));

        assert!(
            out.iter()
                .any(|line| concat_line(line).contains("https://ex.com")),
            "expected URL token to remain present, got: {out:?}"
        );
        assert!(
            !out.iter()
                .any(|line| concat_line(line).contains(long_non_url)),
            "expected long non-url token to wrap on mixed lines, got: {out:?}"
        );
    }

    #[test]
    fn map_owned_wrapped_line_to_range_recovers_on_non_prefix_mismatch() {
        // Match source chars first, then introduce a non-penalty mismatch.
        // The function should recover and return the mapped prefix range.
        let range = map_owned_wrapped_line_to_range("hello world", 0, "helloX", "");
        assert_eq!(range, 0..5);
    }

    #[test]
    fn map_owned_wrapped_line_to_range_indent_coincides_with_source() {
        // When the synthetic indent prefix starts with a character that also
        // appears at the current source position, the mapper must not confuse
        // the indent char for a source match.  Here the indent is "- " and the
        // source text also starts with "-", so a naive char-by-char match would
        // consume the source "-" for the indent "-", set saw_source_char too
        // early, then break on the space — returning 0..1 instead of the full
        // first word.
        let text = "- item one and some more words";
        // Simulate what textwrap would produce for the first continuation line
        // when subsequent_indent = "- ": it prepends "- " to the source slice.
        let range = map_owned_wrapped_line_to_range(text, 0, "- - item one", "- ");
        // The mapper should skip the synthetic "- " prefix and map "- item one"
        // back to source bytes 0..10.
        assert_eq!(range, 0..10);
    }

    #[test]
    fn wrap_ranges_indent_prefix_coincides_with_source_char() {
        // End-to-end: source text starts with the same character as the indent
        // prefix.  wrap_ranges must still reconstruct the full source.
        let text = "- first item is long enough to wrap around";
        let opts = || {
            textwrap::Options::new(16)
                .initial_indent("- ")
                .subsequent_indent("- ")
        };
        let ranges = wrap_ranges(text, opts());
        assert!(!ranges.is_empty());

        let mut rebuilt = String::new();
        let mut cursor = 0usize;
        for range in ranges {
            let start = range.start.max(cursor).min(text.len());
            let end = range.end.min(text.len());
            if start < end {
                rebuilt.push_str(&text[start..end]);
            }
            cursor = cursor.max(end);
        }
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn map_owned_wrapped_line_to_range_repro_overconsumes_repeated_prefix_patterns() {
        let text = "- - foo";
        let opts = textwrap::Options::new(3)
            .initial_indent("- ")
            .subsequent_indent("- ")
            .word_separator(textwrap::WordSeparator::AsciiSpace)
            .break_words(false);
        let wrapped = textwrap::wrap(text, opts);
        let Some(line) = wrapped.first() else {
            panic!("expected at least one wrapped line");
        };

        let mapped = map_owned_wrapped_line_to_range(text, 0, line.as_ref(), "- ");
        let expected_len = line
            .as_ref()
            .strip_prefix("- ")
            .unwrap_or(line.as_ref())
            .len();
        let mapped_len = mapped.end.saturating_sub(mapped.start);
        assert!(
            mapped_len <= expected_len,
            "overconsumed source: text={text:?} line={line:?} mapped={mapped:?} expected_len={expected_len}"
        );
    }

    #[test]
    fn wrap_ranges_recovers_with_non_space_indents() {
        let text = "The quick brown fox jumps over the lazy dog";
        let wrapped = textwrap::wrap(
            text,
            textwrap::Options::new(12)
                .initial_indent("* ")
                .subsequent_indent("  "),
        );
        assert!(
            wrapped
                .iter()
                .any(|line| matches!(line, std::borrow::Cow::Owned(_))),
            "expected textwrap to produce owned lines with synthetic indent prefixes"
        );

        let ranges = wrap_ranges(
            text,
            textwrap::Options::new(12)
                .initial_indent("* ")
                .subsequent_indent("  "),
        );
        assert!(!ranges.is_empty());

        // wrap_ranges returns cursor-oriented ranges that may overlap by one byte;
        // rebuild with cursor progression to validate full source coverage.
        let mut rebuilt = String::new();
        let mut cursor = 0usize;
        for range in ranges {
            let start = range.start.max(cursor).min(text.len());
            let end = range.end.min(text.len());
            if start < end {
                rebuilt.push_str(&text[start..end]);
            }
            cursor = cursor.max(end);
        }

        assert_eq!(rebuilt, text);
    }

    #[test]
    fn wrap_ranges_trim_handles_owned_lines_with_penalty_char() {
        fn split_every_char(word: &str) -> Vec<usize> {
            word.char_indices().skip(1).map(|(idx, _)| idx).collect()
        }

        let text = "a_very_long_token_without_spaces";
        let opts = Options::new(8)
            .word_separator(textwrap::WordSeparator::AsciiSpace)
            .word_splitter(textwrap::WordSplitter::Custom(split_every_char))
            .break_words(false);

        let ranges = wrap_ranges_trim(text, opts);
        let rebuilt = ranges
            .iter()
            .map(|range| &text[range.clone()])
            .collect::<String>();

        assert_eq!(rebuilt, text);
        assert!(ranges.len() > 1, "expected wrapped ranges, got: {ranges:?}");
    }
}
