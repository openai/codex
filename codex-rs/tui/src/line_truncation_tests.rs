use super::line_width;
use super::truncate_line_to_width;
use pretty_assertions::assert_eq;
use ratatui::text::Line;

#[test]
fn halfwidth_sound_marks_stay_with_their_grapheme_when_truncated() {
    let line = Line::from("abｶﾞc");

    assert_eq!(line_width(&line), 5);
    assert_eq!(
        truncate_line_to_width(line.clone(), /*max_width*/ 3),
        Line::from("ab")
    );
    assert_eq!(
        truncate_line_to_width(line, /*max_width*/ 4),
        Line::from("abｶﾞ")
    );
}
