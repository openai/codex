use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::history_cell::PlainHistoryCell;
use pretty_assertions::assert_eq;
use ratatui::text::Line;

fn cell(lines: &[&str]) -> Arc<dyn HistoryCell> {
    Arc::new(PlainHistoryCell::new(
        lines
            .iter()
            .map(|line| Line::from((*line).to_owned()))
            .collect(),
    ))
}

fn render(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) -> Buffer {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 6,
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    buffer
}

fn press(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>], code: KeyCode) {
    assert!(matches!(
        view.handle_key(KeyEvent::new(code, KeyModifiers::NONE), cells),
        Some(ViewAction::Changed)
    ));
}

#[test]
fn entry_starts_on_the_last_selectable_visible_line() {
    for (cells, expected) in [
        (vec![cell(&["top"]), cell(&["bottom"])], "b"),
        (vec![cell(&["text"]), cell(&[""])], "t"),
        (vec![cell(&["abc", "", "", "", "", "", ""])], "c"),
    ] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells);
        assert!(view.begin_select_mode(&cells));
        press(&mut view, &cells, KeyCode::Char('v'));
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
    }

    let cells = vec![cell(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.scroll(&cells, /*rows*/ -2);
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("h"));

    let cells = vec![cell(&["a", "", "", "", "", "", "", "", "", ""])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    assert!(matches!(view.position, Position::Reading(_)));

    let cells = vec![cell(&[""])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(!view.begin_select_mode(&cells));
}

#[test]
fn character_motion_crosses_line_and_entry_boundaries_without_invisible_stops() {
    let cells = vec![cell(&["a", ""]), cell(&[""]), cell(&["", "b"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Left);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("a"));

    press(&mut view, &cells, KeyCode::Esc);
    press(&mut view, &cells, KeyCode::Right);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("b"));

    let cells = vec![cell(&["ab"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    let separator = Anchor {
        key: EntryKey::cell(&cells[0]),
        index: 0,
        offset: 0,
        row_bias: 1,
    };
    view.selection
        .as_mut()
        .expect("selection")
        .set_keyboard_range(separator, separator);
    press(&mut view, &cells, KeyCode::Right);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("a"));

    let wrapped = format!("{} one", "1234567890".repeat(3));
    let cells = vec![cell(&[&wrapped])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Left);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("0"));

    let cells = vec![cell(&["ab"]), cell(&["cd"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    let start = Anchor {
        key: EntryKey::cell(&cells[0]),
        index: 0,
        offset: 0,
        row_bias: 0,
    };
    let end = Anchor {
        key: EntryKey::cell(&cells[1]),
        index: 1,
        offset: 0,
        row_bias: 0,
    };
    let mut selection = Selection::keyboard(view.capture_snapshot(&cells), start);
    selection.set_keyboard_range(start, end);
    view.selection = Some(selection);
    assert!(view.begin_select_mode(&cells));
    assert_eq!(
        view.select_mode
            .and_then(|mode| mode.mark)
            .expect("marked range")
            .active,
        Anchor { offset: 1, ..start }
    );
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ab\n"));
    press(&mut view, &cells, KeyCode::Char('o'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ab\n"));
    press(&mut view, &cells, KeyCode::Char('l'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("b"));

    let cells = vec![cell(&["abc"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    let start = Anchor {
        key: EntryKey::cell(&cells[0]),
        index: 0,
        offset: 1,
        row_bias: 0,
    };
    let end = Anchor { offset: 3, ..start };
    let mut selection = Selection::keyboard(view.capture_snapshot(&cells), start);
    selection.set_keyboard_range(start, end);
    view.selection = Some(selection);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('h'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("b"));
    press(&mut view, &cells, KeyCode::Char('h'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ab"));
}

#[test]
fn vertical_motion_keeps_the_preferred_display_column() {
    let cells = vec![cell(&["abcdef", "x", "abcdef"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    for _ in 0..3 {
        press(&mut view, &cells, KeyCode::Right);
    }
    press(&mut view, &cells, KeyCode::Up);
    press(&mut view, &cells, KeyCode::Up);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("d"));
}

#[test]
fn enter_without_a_mark_exits_but_enter_with_a_mark_copies_and_follows() {
    let cells = vec![cell(&["copy me"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    assert!(matches!(
        view.handle_key(KeyCode::Enter.into(), &cells),
        Some(ViewAction::ExitSelectMode)
    ));

    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('v'));
    assert!(matches!(
        view.handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('v'), KeyModifiers::NONE, KeyEventKind::Repeat,),
            &cells,
        ),
        Some(ViewAction::Changed)
    ));
    assert!(matches!(
        view.handle_key(KeyCode::Enter.into(), &cells),
        Some(ViewAction::CopyAndFollow(text)) if text == "c"
    ));
}

#[test]
fn word_motion_crosses_entries_and_word_end_selects_the_destination_word() {
    let cells = vec![cell(&["alpha"]), cell(&["beta gamma"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Char('j'));

    press(&mut view, &cells, KeyCode::Char('w'));
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Char('e'));

    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta"));
}

#[test]
fn reverse_line_selection_copies_complete_lines_in_document_order() {
    let cells = vec![cell(&["first", "second", "third"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Char('j'));
    press(&mut view, &cells, KeyCode::Char('j'));
    press(&mut view, &cells, KeyCode::Char('V'));
    press(&mut view, &cells, KeyCode::Char('k'));

    assert_eq!(view.selected_text(&cells).as_deref(), Some("second\nthird"));
}

#[test]
fn logical_line_end_handles_empty_lines_and_shifted_dollar() {
    let cells = vec![cell(&["abc", "", "z"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    assert!(matches!(
        view.handle_key(
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::SHIFT),
            &cells,
        ),
        Some(ViewAction::Changed)
    ));
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("c"));

    press(&mut view, &cells, KeyCode::Esc);
    press(&mut view, &cells, KeyCode::Char('j'));
    press(&mut view, &cells, KeyCode::End);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells), None);

    let cells = vec![cell(&["abc", ""])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Char('j'));
    press(&mut view, &cells, KeyCode::End);
}

#[test]
fn configured_pager_page_motion_moves_the_select_cursor() {
    let cells = vec![Arc::new(PlainHistoryCell::new(
        (0..12).map(|row| format!("line {row}").into()).collect(),
    )) as Arc<dyn HistoryCell>];
    let config = serde_json::from_value(serde_json::json!({
        "pager": {"page_down": "x"}
    }))
    .expect("keymap config");
    let keymap = crate::keymap::RuntimeKeymap::from_config(&config).expect("configured page down");
    let mut view = TranscriptView::default();
    view.set_keymap_bindings(&keymap);
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));

    press(&mut view, &cells, KeyCode::Char('v'));
    assert!(view.select_mode.is_some_and(|mode| mode.mark.is_some()));
    press(&mut view, &cells, KeyCode::Char('x'));

    let cursor = view
        .select_mode
        .expect("select mode")
        .cursor(view.selection.as_ref().expect("selection"));
    let layout = view.layout(&cells, cursor.index).expect("cursor layout");
    assert_eq!(
        layout
            .row_for_offset(cursor.offset)
            .saturating_add_signed(cursor.row_bias.saturating_neg()),
        5
    );
}

#[test]
fn transcript_end_character_selection_includes_the_last_grapheme() {
    let cells = vec![cell(&["start"]), cell(&["tail🙂"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('G'));
    press(&mut view, &cells, KeyCode::Char('v'));

    assert_eq!(view.selected_text(&cells).as_deref(), Some("🙂"));
}

#[test]
fn swapping_the_active_end_preserves_text_and_moves_the_shrinking_endpoint() {
    let cells = vec![cell(&["abcdef"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Char('o'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("a"));
    press(&mut view, &cells, KeyCode::Char('o'));
    for _ in 0..3 {
        press(&mut view, &cells, KeyCode::Char('l'));
    }
    assert_eq!(view.selected_text(&cells).as_deref(), Some("abcd"));
    press(&mut view, &cells, KeyCode::Char('o'));
    press(&mut view, &cells, KeyCode::Char('l'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("bcd"));
}

#[test]
fn confirmed_copy_keeps_the_select_mode_caret_on_the_active_grapheme() {
    let cells = vec![cell(&["abc"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Char('l'));
    let Some(ViewAction::Copy(text)) = view.handle_key(KeyCode::Char('y').into(), &cells) else {
        panic!("copy key should copy the marked range");
    };
    assert_eq!(text, "ab");
    assert_eq!(
        view.copy_selected_text_with(
            &cells,
            &text,
            /*clear_selection*/ true,
            |_, _| { Ok(CopyStatus::Confirmed) }
        ),
        Ok(CopyStatus::Confirmed)
    );
    assert!(view.select_mode.is_some());
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("b"));
}
