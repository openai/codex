use super::super::TextArea;
use super::super::TextAreaState;
use super::visible_prefix;
use super::wrapped_lines;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::WidgetRef;

fn wrapped_rows(text: &str, width: u16) -> Vec<&str> {
    wrapped_lines(text, width)
        .into_iter()
        .map(|range| &text[range.start..range.end - 1])
        .collect()
}

#[test]
fn vertical_navigation_preserves_destination_tab_columns() {
    let mut t = TextArea::new();
    t.insert_str("a\tb\nxyz");
    let area = Rect::new(0, 0, /*width*/ 8, /*height*/ 2);
    t.set_cursor(/*pos*/ 5);
    let _ = t.desired_height(area.width);
    t.move_cursor_up();
    assert_eq!((t.cursor(), t.cursor_pos(area)), (1, Some((1, 0))));
    t.move_cursor_down();
    assert_eq!((t.cursor(), t.cursor_pos(area)), (5, Some((1, 1))));
}

#[test]
fn mandatory_breaks_keep_distinct_cursor_positions() {
    let area = Rect::new(0, 0, /*width*/ 4, /*height*/ 3);
    for separator in ["\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}", "\r"] {
        let text = format!("abad{separator}next");
        let rows = wrapped_rows(&text, area.width);
        assert_eq!(rows, ["abad", &format!("{separator}nex"), "t"]);

        let mut t = TextArea::new();
        t.insert_str(&text);
        t.set_cursor(/*pos*/ 4);
        let before = t.cursor_pos(area);
        t.move_cursor_right();
        let after = (t.cursor(), t.cursor_pos(area));
        assert_eq!(
            (before, after),
            (Some((0, 1)), (4 + separator.len(), Some((1, 1))))
        );
        t.move_cursor_left();
        assert_eq!((t.cursor(), t.cursor_pos(area)), (4, before));
        assert_eq!(t.text(), text);

        if separator == "\u{2028}" {
            let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
            terminal
                .draw(|frame| {
                    WidgetRef::render_ref(&(&t), frame.area(), frame.buffer_mut());
                })
                .unwrap();
            insta::assert_snapshot!(
                "mandatory_break_cursor",
                format!(
                    "before: {before:?}\nafter: {after:?}\n{}",
                    terminal.backend()
                )
                .replace(separator, "\\u{2028}")
            );
        }
    }
}

#[test]
fn visible_prefix_bounds_long_hanging_runs_without_splitting_graphemes() {
    let width = 80;
    let count = MAX_USER_INPUT_TEXT_CHARS - 2;
    let area = Rect::new(0, 0, width, /*height*/ 2);
    for separator in [" ", "\t", "\u{2003}", "\u{3000}"] {
        let text = format!("a{}x", separator.repeat(count));
        assert_eq!(
            visible_prefix(&text, width),
            format!("a{}", separator.repeat(usize::from(width)))
        );
        let mut t = TextArea::new();
        t.insert_str(&text);
        t.set_cursor(text.len() - 1 - separator.len());
        assert_eq!(t.cursor_pos(area), Some((0, 1)));
        assert_eq!(t.text(), text);
    }
    assert_eq!(
        visible_prefix("a  \u{301} hidden", /*width*/ 2),
        "a  \u{301}"
    );
}

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
fn breakable_unicode_spaces_hang_before_following_words() {
    for (text, expected_rows) in [
        ("abad abcde", ["abad ", "abcd", "e"]),
        ("abad\u{2003}abcde", ["abad\u{2003}", "abcd", "e"]),
        (
            "abad\u{2003}\u{2003}abcde",
            ["abad\u{2003}\u{2003}", "abcd", "e"],
        ),
        ("abad\u{3000}abcde", ["abad\u{3000}", "abcd", "e"]),
        (
            "abad\u{3000} \u{2003}abcde",
            ["abad\u{3000} \u{2003}", "abcd", "e"],
        ),
    ] {
        assert_eq!(
            wrapped_rows(text, /*width*/ 4),
            expected_rows,
            "text={text:?}"
        );
    }
}

#[test]
fn hanging_spaces_preserve_source_editing_and_visual_navigation() {
    let text = "abad  next";
    let area = Rect::new(0, 0, /*width*/ 4, /*height*/ 3);
    let mut t = TextArea::new();
    t.insert_str(text);
    assert_eq!(t.desired_height(area.width), 3);
    assert_eq!(t.text(), text);

    for cursor in 4..=6 {
        t.set_cursor(cursor);
        assert_eq!(t.cursor_pos(area), Some((0, 1)));
    }
    t.move_cursor_left();
    assert_eq!(t.cursor(), 5);
    t.move_cursor_up();
    assert_eq!(t.cursor(), 0);
    t.move_cursor_down();
    assert_eq!(t.cursor(), 6);
    t.delete_backward(/*n*/ 1);
    assert_eq!(t.text(), "abad next");
    assert_eq!(t.cursor_pos(area), Some((0, 1)));
    t.delete_backward(/*n*/ 1);
    assert_eq!(t.text(), "abadnext");
    assert_eq!(t.cursor_pos(area), Some((0, 1)));
}

#[test]
fn hanging_tabs_keep_cursor_scrolling_and_navigation_aligned() {
    let text = "abc\t a";
    let area = Rect::new(0, 0, /*width*/ 4, /*height*/ 2);
    let mut t = TextArea::new();
    t.insert_str(text);
    let cursors = (0..=text.len())
        .map(|pos| {
            t.set_cursor(pos);
            t.cursor_pos(area)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cursors,
        [(0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (0, 1), (1, 1)].map(Some)
    );

    t.set_cursor(/*pos*/ 4);
    let mut state = TextAreaState::default();
    let mut terminal = Terminal::new(TestBackend::new(area.width, /*height*/ 1)).unwrap();
    terminal
        .draw(|frame| {
            StatefulWidgetRef::render_ref(&(&t), frame.area(), frame.buffer_mut(), &mut state);
        })
        .unwrap();
    let scrolled_cursor = t.cursor_pos_with_state(terminal.backend().buffer().area, state);
    assert_eq!((state.scroll, scrolled_cursor), (1, Some((0, 0))));
    insta::assert_snapshot!(
        "hanging_tab_cursor_and_scroll",
        format!(
            "cursors: {cursors:?}\nscroll: {}\ncursor: {scrolled_cursor:?}\n{}",
            state.scroll,
            terminal.backend()
        )
    );

    t.move_cursor_up();
    assert_eq!((t.cursor(), t.cursor_pos(area)), (0, Some((0, 0))));
    t.move_cursor_down();
    assert_eq!((t.cursor(), t.cursor_pos(area)), (5, Some((0, 1))));
    assert_eq!(t.text(), text);
}

#[test]
fn vertical_navigation_clamps_saved_column_after_resize() {
    let text = "abcdefghij\nabcd  xyz";
    let mut t = TextArea::new();
    t.insert_str(text);
    let mut cursors = Vec::new();
    let mut record = |t: &TextArea, label: &str, width: u16, expected| {
        let area = Rect::new(0, 0, width, t.desired_height(width));
        let cursor = (t.cursor(), t.cursor_pos(area));
        assert_eq!(cursor, expected, "{label}");
        cursors.push(format!("{label}: {cursor:?}"));
    };

    let _ = t.desired_height(/*width*/ 10);
    t.move_cursor_up();
    record(&t, "up at width 10", /*width*/ 10, (10, Some((0, 1))));
    record(
        &t,
        "resize to width 4",
        /*width*/ 4,
        (10, Some((2, 2))),
    );
    t.move_cursor_down();
    record(&t, "down to abcd", /*width*/ 4, (14, Some((3, 3))));
    t.move_cursor_down();
    record(&t, "down to xyz", /*width*/ 4, (20, Some((3, 4))));
    t.move_cursor_up();
    record(&t, "up to abcd", /*width*/ 4, (14, Some((3, 3))));
    t.move_cursor_up();
    record(&t, "up to ij", /*width*/ 4, (10, Some((2, 2))));
    let _ = t.desired_height(/*width*/ 10);
    t.move_cursor_down();
    record(
        &t,
        "restore width 10",
        /*width*/ 10,
        (20, Some((9, 2))),
    );

    assert_eq!(t.text(), text);
    let layouts = [4, 10].map(|width| {
        let mut terminal = Terminal::new(TestBackend::new(width, t.desired_height(width))).unwrap();
        terminal
            .draw(|frame| {
                WidgetRef::render_ref(&(&t), frame.area(), frame.buffer_mut());
            })
            .unwrap();
        format!("width {width}\n{}", terminal.backend())
    });
    insta::assert_snapshot!(
        "vertical_navigation_after_resize",
        format!("{}\n\n{}", cursors.join("\n"), layouts.join("\n"))
    );
}

#[test]
fn typing_after_eol_spaces_reflows_without_changing_text() {
    let area = Rect::new(0, 0, /*width*/ 4, /*height*/ 2);
    let mut t = TextArea::new();
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let mut snapshots = Vec::new();
    for (label, input, expected_cursor) in [
        ("full line", "abad", (0, 1)),
        ("trailing spaces", "   ", (3, 1)),
        ("following word", "x", (1, 1)),
        ("delete following word", "", (3, 1)),
    ] {
        if input.is_empty() {
            t.delete_backward(/*n*/ 1);
        } else {
            t.insert_str(input);
        }
        terminal
            .draw(|frame| {
                WidgetRef::render_ref(&(&t), frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_eq!(t.cursor_pos(area), Some(expected_cursor));
        snapshots.push(format!(
            "{label}\ntext: {:?}\ncursor: {expected_cursor:?}\n{}",
            t.text(),
            terminal.backend()
        ));
    }
    assert_eq!(t.text(), "abad   ");
    insta::assert_snapshot!("typing_after_eol_spaces", snapshots.join("\n\n"));
}

#[test]
fn explicit_indentation_and_trailing_spaces_do_not_hang() {
    for (text, expected_rows) in [
        ("abad\n a", vec!["abad", "", " a"]),
        ("abad \n a", vec!["abad", " ", " a"]),
        ("    a", vec!["    ", "a"]),
        ("abad  ", vec!["abad", "  "]),
        ("abad\u{2003}", vec!["abad", "\u{2003}"]),
        ("abad\u{3000}\n a", vec!["abad", "\u{3000}", " a"]),
    ] {
        assert_eq!(
            wrapped_rows(text, /*width*/ 4),
            expected_rows,
            "text={text:?}"
        );
    }
}

#[test]
fn nonbreaking_spaces_never_hang() {
    for space in ['\u{a0}', '\u{2007}', '\u{202f}'] {
        let text = format!("abcd{space} x");
        let last = format!("{space} x");
        assert_eq!(wrapped_rows(&text, /*width*/ 4), ["abcd", last.as_str()]);
    }
}

#[test]
fn prose_wraps_like_the_queued_input_preview() {
    for text in [
        "new access tokens",
        "aaaa   next",
        "a foo-barbaz",
        "some ordinary prose",
    ] {
        for width in 3..=16 {
            let composer = wrapped_rows(text, width)
                .into_iter()
                .map(|row| row.trim_end_matches(' '))
                .filter(|row| !row.is_empty())
                .collect::<Vec<_>>();
            let preview =
                adaptive_wrap_lines([Line::from(text)], RtOptions::new(usize::from(width)))
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>();
            assert_eq!(composer, preview, "text={text:?}, width={width}");
        }
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
                        display_width(row.trim_end_matches(' ')) <= usize::from(width),
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
                                || (position.1 == previous.1 && position.0 > previous.0)
                                || (position == previous && text.as_bytes()[cursor - 1] == b' '),
                            "text={text:?}, width={width}, ranges={ranges:?}, cursor={cursor}, previous={previous:?}, position={position:?}"
                        );
                    }
                    previous = Some(position);
                }
            }
        }
    }
}
