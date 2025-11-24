//! Functions for filling text.

use crate::{wrap, wrap_algorithms, Options, WordSeparator};

/// Fill a line of text at a given width.
///
/// The result is a [`String`], complete with newlines between each
/// line. Use [`wrap()`] if you need access to the individual lines.
///
/// The easiest way to use this function is to pass an integer for
/// `width_or_options`:
///
/// ```
/// use textwrap::fill;
///
/// assert_eq!(
///     fill("Memory safety without garbage collection.", 15),
///     "Memory safety\nwithout garbage\ncollection."
/// );
/// ```
///
/// If you need to customize the wrapping, you can pass an [`Options`]
/// instead of an `usize`:
///
/// ```
/// use textwrap::{fill, Options};
///
/// let options = Options::new(15)
///     .initial_indent("- ")
///     .subsequent_indent("  ");
/// assert_eq!(
///     fill("Memory safety without garbage collection.", &options),
///     "- Memory safety\n  without\n  garbage\n  collection."
/// );
/// ```
pub fn fill<'a, Opt>(text: &str, width_or_options: Opt) -> String
where
    Opt: Into<Options<'a>>,
{
    let options = width_or_options.into();

    if text.len() < options.width && !text.contains('\n') && options.initial_indent.is_empty() {
        String::from(text.trim_end_matches(' '))
    } else {
        fill_slow_path(text, options)
    }
}

/// Slow path for fill.
///
/// This is taken when `text` is longer than `options.width`.
pub(crate) fn fill_slow_path(text: &str, options: Options<'_>) -> String {
    // This will avoid reallocation in simple cases (no
    // indentation, no hyphenation).
    let mut result = String::with_capacity(text.len());

    let line_ending_str = options.line_ending.as_str();
    for (i, line) in wrap(text, options).iter().enumerate() {
        if i > 0 {
            result.push_str(line_ending_str);
        }
        result.push_str(line);
    }

    result
}

/// Fill `text` in-place without reallocating the input string.
///
/// This function works by modifying the input string: some `' '`
/// characters will be replaced by `'\n'` characters. The rest of the
/// text remains untouched.
///
/// Since we can only replace existing whitespace in the input with
/// `'\n'` (there is no space for `"\r\n"`), we cannot do hyphenation
/// nor can we split words longer than the line width. We also need to
/// use `AsciiSpace` as the word separator since we need `' '`
/// characters between words in order to replace some of them with a
/// `'\n'`. Indentation is also ruled out. In other words,
/// `fill_inplace(width)` behaves as if you had called [`fill()`] with
/// these options:
///
/// ```
/// # use textwrap::{core, LineEnding, Options, WordSplitter, WordSeparator, WrapAlgorithm};
/// # let width = 80;
/// Options::new(width)
///     .break_words(false)
///     .line_ending(LineEnding::LF)
///     .word_separator(WordSeparator::AsciiSpace)
///     .wrap_algorithm(WrapAlgorithm::FirstFit)
///     .word_splitter(WordSplitter::NoHyphenation);
/// ```
///
/// The wrap algorithm is
/// [`WrapAlgorithm::FirstFit`](crate::WrapAlgorithm::FirstFit) since
/// this is the fastest algorithm — and the main reason to use
/// `fill_inplace` is to get the string broken into newlines as fast
/// as possible.
///
/// A last difference is that (unlike [`fill()`]) `fill_inplace` can
/// leave trailing whitespace on lines. This is because we wrap by
/// inserting a `'\n'` at the final whitespace in the input string:
///
/// ```
/// let mut text = String::from("Hello   World!");
/// textwrap::fill_inplace(&mut text, 10);
/// assert_eq!(text, "Hello  \nWorld!");
/// ```
///
/// If we didn't do this, the word `World!` would end up being
/// indented. You can avoid this if you make sure that your input text
/// has no double spaces.
///
/// # Performance
///
/// In benchmarks, `fill_inplace` is about twice as fast as
/// [`fill()`]. Please see the [`linear`
/// benchmark](https://github.com/mgeisler/textwrap/blob/master/benchmarks/linear.rs)
/// for details.
pub fn fill_inplace(text: &mut String, width: usize) {
    let mut indices = Vec::new();

    let mut offset = 0;
    for line in text.split('\n') {
        let words = WordSeparator::AsciiSpace
            .find_words(line)
            .collect::<Vec<_>>();
        let wrapped_words = wrap_algorithms::wrap_first_fit(&words, &[width as f64]);

        let mut line_offset = offset;
        for words in &wrapped_words[..wrapped_words.len() - 1] {
            let line_len = words
                .iter()
                .map(|word| word.len() + word.whitespace.len())
                .sum::<usize>();

            line_offset += line_len;
            // We've advanced past all ' ' characters -- want to move
            // one ' ' backwards and insert our '\n' there.
            indices.push(line_offset - 1);
        }

        // Advance past entire line, plus the '\n' which was removed
        // by the split call above.
        offset += line.len() + 1;
    }

    let mut bytes = std::mem::take(text).into_bytes();
    for idx in indices {
        bytes[idx] = b'\n';
    }
    *text = String::from_utf8(bytes).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WrapAlgorithm;

    #[test]
    fn fill_simple() {
        assert_eq!(fill("foo bar baz", 10), "foo bar\nbaz");
    }

    #[test]
    fn fill_unicode_boundary() {
        // https://github.com/mgeisler/textwrap/issues/390
        fill("\u{1b}!Ͽ", 10);
    }

    #[test]
    fn non_breaking_space() {
        let options = Options::new(5).break_words(false);
        assert_eq!(fill("foo bar baz", &options), "foo bar baz");
    }

    #[test]
    fn non_breaking_hyphen() {
        let options = Options::new(5).break_words(false);
        assert_eq!(fill("foo‑bar‑baz", &options), "foo‑bar‑baz");
    }

    #[test]
    fn fill_preserves_line_breaks_trims_whitespace() {
        assert_eq!(fill("  ", 80), "");
        assert_eq!(fill("  \n  ", 80), "\n");
        assert_eq!(fill("  \n \n  \n ", 80), "\n\n\n");
    }

    #[test]
    fn preserve_line_breaks() {
        assert_eq!(fill("", 80), "");
        assert_eq!(fill("\n", 80), "\n");
        assert_eq!(fill("\n\n\n", 80), "\n\n\n");
        assert_eq!(fill("test\n", 80), "test\n");
        assert_eq!(fill("test\n\na\n\n", 80), "test\n\na\n\n");
        assert_eq!(
            fill(
                "1 3 5 7\n1 3 5 7",
                Options::new(7).wrap_algorithm(WrapAlgorithm::FirstFit)
            ),
            "1 3 5 7\n1 3 5 7"
        );
        assert_eq!(
            fill(
                "1 3 5 7\n1 3 5 7",
                Options::new(5).wrap_algorithm(WrapAlgorithm::FirstFit)
            ),
            "1 3 5\n7\n1 3 5\n7"
        );
    }

    #[test]
    fn break_words_line_breaks() {
        assert_eq!(fill("ab\ncdefghijkl", 5), "ab\ncdefg\nhijkl");
        assert_eq!(fill("abcdefgh\nijkl", 5), "abcde\nfgh\nijkl");
    }

    #[test]
    fn break_words_empty_lines() {
        assert_eq!(
            fill("foo\nbar", &Options::new(2).break_words(false)),
            "foo\nbar"
        );
    }

    #[test]
    fn fill_inplace_empty() {
        let mut text = String::from("");
        fill_inplace(&mut text, 80);
        assert_eq!(text, "");
    }

    #[test]
    fn fill_inplace_simple() {
        let mut text = String::from("foo bar baz");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "foo bar\nbaz");
    }

    #[test]
    fn fill_inplace_multiple_lines() {
        let mut text = String::from("Some text to wrap over multiple lines");
        fill_inplace(&mut text, 12);
        assert_eq!(text, "Some text to\nwrap over\nmultiple\nlines");
    }

    #[test]
    fn fill_inplace_long_word() {
        let mut text = String::from("Internationalization is hard");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "Internationalization\nis hard");
    }

    #[test]
    fn fill_inplace_no_hyphen_splitting() {
        let mut text = String::from("A well-chosen example");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "A\nwell-chosen\nexample");
    }

    #[test]
    fn fill_inplace_newlines() {
        let mut text = String::from("foo bar\n\nbaz\n\n\n");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "foo bar\n\nbaz\n\n\n");
    }

    #[test]
    fn fill_inplace_newlines_reset_line_width() {
        let mut text = String::from("1 3 5\n1 3 5 7 9\n1 3 5 7 9 1 3");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "1 3 5\n1 3 5 7 9\n1 3 5 7 9\n1 3");
    }

    #[test]
    fn fill_inplace_leading_whitespace() {
        let mut text = String::from("  foo bar baz");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "  foo bar\nbaz");
    }

    #[test]
    fn fill_inplace_trailing_whitespace() {
        let mut text = String::from("foo bar baz  ");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "foo bar\nbaz  ");
    }

    #[test]
    fn fill_inplace_interior_whitespace() {
        // To avoid an unwanted indentation of "baz", it is important
        // to replace the final ' ' with '\n'.
        let mut text = String::from("foo  bar    baz");
        fill_inplace(&mut text, 10);
        assert_eq!(text, "foo  bar   \nbaz");
    }
}
