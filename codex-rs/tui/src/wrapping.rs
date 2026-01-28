use std::borrow::Cow;
use std::ops::Range;
use textwrap::Options;

use crate::render::line_utils::push_owned_lines;
use crate::render::model::RenderCell;
use crate::render::model::RenderLine;
use crate::render::model::RenderStyle;

pub(crate) fn wrap_ranges<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    let mut lines: Vec<Range<usize>> = Vec::new();
    for line in textwrap::wrap(text, opts).iter() {
        match line {
            Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                let trailing_spaces = text[end..].chars().take_while(|c| *c == ' ').count();
                lines.push(start..end + trailing_spaces + 1);
            }
            Cow::Owned(_) => panic!("wrap_ranges: unexpected owned string"),
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
    for line in textwrap::wrap(text, opts).iter() {
        match line {
            Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                lines.push(start..end);
            }
            Cow::Owned(_) => panic!("wrap_ranges_trim: unexpected owned string"),
        }
    }
    lines
}

#[derive(Debug, Clone)]
pub struct RtOptions {
    /// The width in columns at which the text will be wrapped.
    pub width: usize,
    /// Line ending used for breaking lines.
    pub line_ending: textwrap::LineEnding,
    /// Indentation used for the first line of output. See the
    /// [`Options::initial_indent`] method.
    pub initial_indent: RenderLine,
    /// Indentation used for subsequent lines of output. See the
    /// [`Options::subsequent_indent`] method.
    pub subsequent_indent: RenderLine,
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
impl From<usize> for RtOptions {
    fn from(width: usize) -> Self {
        RtOptions::new(width)
    }
}

impl RtOptions {
    pub fn new(width: usize) -> Self {
        RtOptions {
            width,
            line_ending: textwrap::LineEnding::LF,
            initial_indent: RenderLine::default(),
            subsequent_indent: RenderLine::default(),
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

    pub fn initial_indent(self, initial_indent: RenderLine) -> Self {
        RtOptions {
            initial_indent,
            ..self
        }
    }

    pub fn subsequent_indent(self, subsequent_indent: RenderLine) -> Self {
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

    pub fn word_separator(self, word_separator: textwrap::WordSeparator) -> RtOptions {
        RtOptions {
            word_separator,
            ..self
        }
    }

    pub fn wrap_algorithm(self, wrap_algorithm: textwrap::WrapAlgorithm) -> RtOptions {
        RtOptions {
            wrap_algorithm,
            ..self
        }
    }

    pub fn word_splitter(self, word_splitter: textwrap::WordSplitter) -> RtOptions {
        RtOptions {
            word_splitter,
            ..self
        }
    }
}

#[must_use]
pub(crate) fn word_wrap_line<O>(line: &RenderLine, width_or_options: O) -> Vec<RenderLine>
where
    O: Into<RtOptions>,
{
    // Flatten the line and record cell byte ranges.
    let mut flat = String::new();
    let mut span_bounds = Vec::new();
    let mut acc = 0usize;
    for cell in &line.spans {
        let text = cell.content.as_str();
        let start = acc;
        flat.push_str(text);
        acc += text.len();
        span_bounds.push((start..acc, cell.style));
    }

    let rt_opts: RtOptions = width_or_options.into();
    let opts = Options::new(rt_opts.width)
        .line_ending(rt_opts.line_ending)
        .break_words(rt_opts.break_words)
        .wrap_algorithm(rt_opts.wrap_algorithm)
        .word_separator(rt_opts.word_separator)
        .word_splitter(rt_opts.word_splitter);

    let mut out: Vec<RenderLine> = Vec::new();

    // Compute first line range with reduced width due to initial indent.
    let initial_width_available = opts
        .width
        .saturating_sub(rt_opts.initial_indent.width())
        .max(1);
    let initial_wrapped = wrap_ranges_trim(&flat, opts.clone().width(initial_width_available));
    let Some(first_line_range) = initial_wrapped.first() else {
        return vec![rt_opts.initial_indent];
    };

    // Build first wrapped line with initial indent.
    {
        let mut cells = rt_opts.initial_indent.spans.clone();
        cells.extend(slice_line_cells(&flat, &span_bounds, first_line_range));
        out.push(RenderLine::new(cells));
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
        let mut cells = rt_opts.subsequent_indent.spans.clone();
        let offset_range = (r.start + base)..(r.end + base);
        cells.extend(slice_line_cells(&flat, &span_bounds, &offset_range));
        out.push(RenderLine::new(cells));
    }

    out
}

/// Utilities to allow wrapping either borrowed or owned lines.
#[derive(Debug)]
enum LineInput<'a> {
    Borrowed(&'a RenderLine),
    Owned(RenderLine),
}

impl<'a> LineInput<'a> {
    fn as_ref(&self) -> &RenderLine {
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

impl<'a> IntoLineInput<'a> for &'a RenderLine {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for &'a mut RenderLine {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for RenderLine {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(self)
    }
}

impl<'a> IntoLineInput<'a> for String {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(RenderLine::from(self))
    }
}

impl<'a> IntoLineInput<'a> for &'a str {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(RenderLine::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Cow<'a, str> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(RenderLine::from(self.into_owned()))
    }
}

/// Wrap a sequence of lines, applying the initial indent only to the very first
/// output line, and using the subsequent indent for all later wrapped pieces.
#[allow(private_bounds)] // IntoLineInput isn't public, but it doesn't really need to be.
pub(crate) fn word_wrap_lines<'a, I, O, L>(lines: I, width_or_options: O) -> Vec<RenderLine>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
    O: Into<RtOptions>,
{
    let base_opts: RtOptions = width_or_options.into();
    let mut out: Vec<RenderLine> = Vec::new();

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

pub(crate) fn word_wrap_lines_borrowed<'a, I, O>(lines: I, width_or_options: O) -> Vec<RenderLine>
where
    I: IntoIterator<Item = &'a RenderLine>,
    O: Into<RtOptions>,
{
    let base_opts: RtOptions = width_or_options.into();
    let mut out: Vec<RenderLine> = Vec::new();
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

fn slice_line_cells(
    original: &str,
    span_bounds: &[(Range<usize>, RenderStyle)],
    range: &Range<usize>,
) -> Vec<RenderCell> {
    let start_byte = range.start;
    let end_byte = range.end;
    let mut acc: Vec<RenderCell> = Vec::new();
    for (range, style) in span_bounds {
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
            let content = &original[seg_start..seg_end];
            acc.push(RenderCell::new(content.to_string(), *style));
        }
        if e >= end_byte {
            break;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn concat_line(line: &RenderLine) -> String {
        line.spans
            .iter()
            .map(|cell| cell.content.as_str())
            .collect::<String>()
    }

    #[test]
    fn trivial_unstyled_no_indents_wide_width() {
        let line = RenderLine::from("hello");
        let out = word_wrap_line(&line, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "hello");
    }

    #[test]
    fn simple_unstyled_wrap_narrow_width() {
        let line = RenderLine::from("hello world");
        let out = word_wrap_line(&line, 5);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn simple_styled_wrap_preserves_styles() {
        let line = RenderLine::from(vec![
            RenderCell::color("hello ", crate::render::model::RenderColor::Red),
            RenderCell::plain("world"),
        ]);
        let out = word_wrap_line(&line, 6);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(out[0].spans.len(), 1);
        assert_eq!(
            out[0].spans[0].style.fg,
            Some(crate::render::model::RenderColor::Red)
        );
        assert_eq!(concat_line(&out[1]), "world");
        assert_eq!(out[1].spans.len(), 1);
        assert_eq!(out[1].spans[0].style.fg, None);
    }

    #[test]
    fn with_initial_and_subsequent_indents() {
        let opts = RtOptions::new(8)
            .initial_indent(RenderLine::from("- "))
            .subsequent_indent(RenderLine::from("  "));
        let line = RenderLine::from("hello world foo");
        let out = word_wrap_line(&line, opts);
        assert!(concat_line(&out[0]).starts_with("- "));
        assert!(concat_line(&out[1]).starts_with("  "));
        assert!(concat_line(&out[2]).starts_with("  "));
        assert_eq!(concat_line(&out[0]), "- hello");
        assert_eq!(concat_line(&out[1]), "  world");
        assert_eq!(concat_line(&out[2]), "  foo");
    }

    #[test]
    fn empty_initial_indent_subsequent_spaces() {
        let opts = RtOptions::new(8)
            .initial_indent(RenderLine::from(""))
            .subsequent_indent(RenderLine::from("    "));
        let line = RenderLine::from("hello world foobar");
        let out = word_wrap_line(&line, opts);
        assert!(concat_line(&out[0]).starts_with("hello"));
        for l in &out[1..] {
            assert!(concat_line(l).starts_with("    "));
        }
    }

    #[test]
    fn empty_input_yields_single_empty_line() {
        let line = RenderLine::from("");
        let out = word_wrap_line(&line, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "");
    }

    #[test]
    fn leading_spaces_preserved_on_first_line() {
        let line = RenderLine::from("   hello");
        let out = word_wrap_line(&line, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "   hello");
    }

    #[test]
    fn multiple_spaces_between_words_dont_start_next_line_with_spaces() {
        let line = RenderLine::from("hello   world");
        let out = word_wrap_line(&line, 8);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn break_words_false_allows_overflow_for_long_word() {
        let opts = RtOptions::new(5).break_words(false);
        let line = RenderLine::from("supercalifragilistic");
        let out = word_wrap_line(&line, opts);
        assert_eq!(out.len(), 1);
        assert_eq!(concat_line(&out[0]), "supercalifragilistic");
    }

    #[test]
    fn hyphen_splitter_breaks_at_hyphen() {
        let line = RenderLine::from("hello-world");
        let out = word_wrap_line(&line, 7);
        assert_eq!(out.len(), 2);
        assert_eq!(concat_line(&out[0]), "hello-");
        assert_eq!(concat_line(&out[1]), "world");
    }

    #[test]
    fn indent_consumes_width_leaving_one_char_space() {
        let opts = RtOptions::new(3)
            .initial_indent(RenderLine::from("  "))
            .subsequent_indent(RenderLine::from("  "));
        let line = RenderLine::from("a b c");
        let out = word_wrap_line(&line, opts);
        assert_eq!(concat_line(&out[0]), "  a");
        assert_eq!(concat_line(&out[1]), "  b");
        assert_eq!(concat_line(&out[2]), "  c");
    }
}
