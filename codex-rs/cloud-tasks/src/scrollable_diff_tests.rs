use super::ScrollableDiff;
use pretty_assertions::assert_eq;

#[test]
fn wraps_halfwidth_sound_marks_using_ratatui_width() {
    let mut diff = ScrollableDiff::new();
    diff.set_content(vec![
        "aｶﾞbﾊﾟc".to_string(),
        "a ﾞb".to_string(),
        "a ﾟb".to_string(),
    ]);
    diff.set_width(/*width*/ 2);

    insta::assert_snapshot!(
        diff.wrapped_lines().join("\n"),
        @r"
        a
        ｶﾞ
        b
        ﾊﾟ
        c
        a
        ﾞb
        a
        ﾟb
        "
    );
    assert_eq!(diff.wrapped_src_indices(), [0, 0, 0, 0, 0, 1, 1, 2, 2]);
}

#[test]
fn does_not_emit_empty_rows_after_full_width_sound_mark_graphemes() {
    for grapheme in ["ｶﾞ", "ﾊﾟ"] {
        let mut diff = ScrollableDiff::new();
        diff.set_content(vec![format!("a {grapheme} tail")]);
        diff.set_width(/*width*/ 2);

        let expected = ["a", grapheme, "ta", "il"];
        let actual = diff
            .wrapped_lines()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(diff.wrapped_src_indices(), [0, 0, 0, 0]);
        assert_eq!(diff.state.content_h, 4);
    }
}
