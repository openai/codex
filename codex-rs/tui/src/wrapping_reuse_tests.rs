//! Compare reused first-fit rows with the generic wrapping path, including source provenance.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;

#[test]
fn custom_word_separator_reprocesses_the_remainder() {
    let separator = WordSeparator::Custom(|text| {
        let Some(space) = text.find(' ') else {
            return Box::new(std::iter::once(Word::from(text)));
        };
        Box::new([Word::from(&text[..=space]), Word::from(&text[space + 1..])].into_iter())
    });
    let line = Line::from("one two three");
    let rows =
        word_wrap_line_with_source(&line, RtOptions::new(/*width*/ 5).word_separator(separator))
            .into_iter()
            .map(|row| (row.line, row.range))
            .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            (Line::from("one"), 0..3),
            (Line::from("two"), 4..7),
            (Line::from("three"), 8..13),
        ]
    );
}

#[test]
fn first_fit_reuse_preserves_styled_rows_and_source_ranges() {
    // A custom first-fit function exercises the unchanged generic path without copying
    // its implementation into the test. Compare complete rows, styles and byte ranges.
    let generic_first_fit = textwrap::WrapAlgorithm::Custom(|words, widths| {
        textwrap::WrapAlgorithm::FirstFit.wrap(words, widths)
    });
    // Cover distinct split, width, indentation and separator boundaries without a random matrix.
    for (text, width, initial, subsequent, break_words, separator) in [
        ("", 0, "", "", true, WordSeparator::AsciiSpace),
        (
            "hello world",
            0,
            "> ",
            "  ",
            true,
            WordSeparator::AsciiSpace,
        ),
        (
            "cafe\u{301} 中文 👩🏽‍💻 🇧🇷 ｶﾞ",
            2,
            "",
            "",
            true,
            WordSeparator::UnicodeBreakProperties,
        ),
        (
            "a-very-long-hyphenated-word",
            5,
            "界",
            "ab",
            true,
            WordSeparator::AsciiSpace,
        ),
        (
            "https://example.com/a/b tail",
            17,
            "> ",
            "  ",
            false,
            WordSeparator::AsciiSpace,
        ),
        (
            "alpha  beta\u{a0}gamma\u{200b}delta",
            5,
            "",
            "  ",
            true,
            WordSeparator::UnicodeBreakProperties,
        ),
        (
            "one\n two\r\nthree\tfour",
            5,
            "> ",
            "  ",
            true,
            WordSeparator::AsciiSpace,
        ),
        (
            "\u{1b}[31mred\u{1b}[0m tail",
            80,
            "",
            "",
            false,
            WordSeparator::AsciiSpace,
        ),
    ] {
        let line = Line::from(vec![text.bold(), " suffix".cyan()]).italic();
        let options = RtOptions::new(width)
            .initial_indent(Line::from(initial.red()))
            .subsequent_indent(Line::from(subsequent.green()))
            .break_words(break_words)
            .word_separator(separator);
        let actual = word_wrap_line_with_source(&line, options.clone())
            .into_iter()
            .map(|row| (row.line, row.range, row.prefix_bytes))
            .collect::<Vec<_>>();
        let expected = word_wrap_line_with_source(&line, options.wrap_algorithm(generic_first_fit))
            .into_iter()
            .map(|row| (row.line, row.range, row.prefix_bytes))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "text={text:?}, width={width}");
    }
}
