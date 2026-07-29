use super::truncate_line_word_boundary_with_ellipsis;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

#[test]
fn halfwidth_sound_marks_are_truncated_at_a_grapheme_boundary() {
    let line = Line::from("abｶﾞ tail");

    assert_eq!(
        truncate_line_word_boundary_with_ellipsis(line, /*max_width*/ 4),
        Line::from(vec![Span::raw("ab"), Span::raw("…")])
    );
}

#[test]
fn halfwidth_sound_marks_are_truncated_and_rendered_at_a_grapheme_boundary() {
    let lines = [
        Line::from(vec!["ab".bold(), "ｶﾞ".cyan(), " tail".dim()]),
        Line::from(vec!["xy".bold(), "ﾊﾟ".magenta(), " tail".dim()]),
    ]
    .map(|line| truncate_line_word_boundary_with_ellipsis(line, /*max_width*/ 5));
    let area = Rect::new(0, 0, 5, 2);
    let mut buf = Buffer::empty(area);

    Paragraph::new(lines.to_vec()).render(area, &mut buf);

    insta::assert_snapshot!(
        "request_user_input_halfwidth_sound_marks_truncation",
        format!("{buf:?}\n{lines:#?}")
    );
}
