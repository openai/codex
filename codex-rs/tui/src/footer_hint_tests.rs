use super::wrap_hint_rows;
use pretty_assertions::assert_eq;
use unicode_width::UnicodeWidthStr;

#[test]
fn wraps_whole_hints_using_display_width() {
    let hints = ["↑ 上", "↓ 下", "enter select"];
    assert_eq!(
        wrap_hint_rows(
            hints,
            /*width*/ 9,
            /*separator_width*/ 1,
            |hint| { UnicodeWidthStr::width(*hint) }
        ),
        vec![vec!["↑ 上", "↓ 下"], vec!["enter select"]]
    );
}

#[test]
fn retains_oversized_hints_and_an_empty_row() {
    assert_eq!(
        wrap_hint_rows(
            ["ctrl+x stop", "esc back"],
            /*width*/ 0,
            /*separator_width*/ 3,
            |hint| hint.len()
        ),
        vec![vec!["ctrl+x stop"], vec!["esc back"]]
    );
    assert_eq!(
        wrap_hint_rows(
            Vec::<String>::new(),
            /*width*/ 40,
            /*separator_width*/ 3,
            String::len
        ),
        vec![Vec::<String>::new()]
    );
}

#[test]
fn inactive_hints_keep_controls_and_measure_only_visible_items() {
    use super::Hint;
    use super::HintFocus;
    let focus = HintFocus::Focusable(
        crate::key_hint::plain(crossterm::event::KeyCode::Char('s')).into(),
        "focus",
    );
    let hints = || {
        [
            Hint::Button {
                key: Some("enter".into()),
                label: "open",
                action: 1,
                before_focus: false,
            },
            Hint::Button {
                key: None,
                label: "返回",
                action: 2,
                before_focus: true,
            },
            Hint::Key("↓".into(), "move".into()),
        ]
    };
    let row = focus.render_row(hints(), /*width*/ 17);
    assert_eq!(row.line.to_string(), "返回 · s focus …");
    assert_eq!(row.targets, vec![(0..4, 2)]);
    let rows = focus.render_hints(hints(), /*width*/ 14, /*max_rows*/ 2);
    assert_eq!(rows[0].targets, vec![(0..4, 2)]);
    assert_eq!(rows[1].targets, vec![(0..4, 1)]);
}
