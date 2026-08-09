use super::super::TextArea;
use super::wrapped_lines;
use crate::width::display_width;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;

#[test]
fn preserves_textwrap_word_boundaries() {
    for (text, width, expected_rows) in [
        ("a foo-barbaz", 10, ["a foo-", "barbaz"]),
        ("a foo-barbazqux", 10, ["a foo-", "barbazqux"]),
        ("a café-barbaz", 10, ["a café-", "barbaz"]),
        ("a foo/barbaz", 10, ["a foo/", "barbaz"]),
        ("a foo—barbaz", 10, ["a foo—", "barbaz"]),
        ("a abc\u{a0}de", 7, ["a ", "abc\u{a0}de"]),
        ("a abc\u{2007}de", 7, ["a ", "abc\u{2007}de"]),
        ("a abc\u{202f}de", 7, ["a ", "abc\u{202f}de"]),
        ("a \u{a0}abc", 5, ["a ", "\u{a0}abc"]),
    ] {
        let rows = wrapped_lines(text, width)
            .iter()
            .map(|range| &text[range.start..range.end - 1])
            .collect::<Vec<_>>();

        assert_eq!(rows, expected_rows, "text={text:?}, width={width}");
    }
}

#[test]
fn breakable_unicode_spaces_stay_with_following_words() {
    for (text, expected_rows) in [
        ("abad abcde", ["abad", " abc", "de"]),
        ("abad\u{2003}abcde", ["abad", "\u{2003}abc", "de"]),
        ("abad\u{3000}abcde", ["abad", "\u{3000}ab", "cde"]),
    ] {
        let rows = wrapped_lines(text, /*width*/ 4)
            .iter()
            .map(|range| &text[range.start..range.end - 1])
            .collect::<Vec<_>>();

        assert_eq!(rows, expected_rows, "text={text:?}");
    }
}

#[test]
fn wraps_maximum_length_unbroken_word_in_one_pass() {
    let text = "x".repeat(MAX_USER_INPUT_TEXT_CHARS);
    let width = 80;
    let ranges = wrapped_lines(&text, width);

    assert_eq!(
        ranges.len(),
        MAX_USER_INPUT_TEXT_CHARS.div_ceil(usize::from(width))
    );
    assert_eq!(ranges.first(), Some(&(0..usize::from(width) + 1)));
    assert_eq!(
        ranges.last().map(|range| range.end),
        Some(MAX_USER_INPUT_TEXT_CHARS + 1)
    );
}

#[test]
fn ascii_wrapped_rows_fit_and_preserve_cursor_positions() {
    for len in 0_u32..=7 {
        for mut encoded in 0..4_usize.pow(len) {
            let mut text = String::with_capacity(len as usize);
            for _ in 0..len {
                text.push([' ', 'a', 'b', '-'][encoded % 4]);
                encoded /= 4;
            }

            for width in 1_u16..=5 {
                let mut t = TextArea::new();
                t.insert_str(&text);
                let ranges = t.wrapped_lines(width).to_vec();
                let mut end = 0;
                for range in &ranges {
                    assert_eq!(range.start, end, "text={text:?}, width={width}");
                    end = range.end - 1;
                    let row = &text[range.start..end];
                    assert!(
                        display_width(row) <= usize::from(width),
                        "text={text:?}, width={width}, row={row:?}"
                    );
                }
                assert_eq!(end, text.len(), "text={text:?}, width={width}");

                let area = Rect::new(0, 0, width, text.len() as u16 + 1);
                let mut previous: Option<(u16, u16)> = None;
                for cursor in 0..=text.len() {
                    t.set_cursor(cursor);
                    let position = t.cursor_pos(area).unwrap();
                    if let Some(previous) = previous {
                        assert!(
                            position.1 > previous.1
                                || (position.1 == previous.1 && position.0 > previous.0),
                            "text={text:?}, width={width}, ranges={ranges:?}, cursor={cursor}, previous={previous:?}, position={position:?}"
                        );
                    }
                    previous = Some(position);
                }
            }
        }
    }
}
