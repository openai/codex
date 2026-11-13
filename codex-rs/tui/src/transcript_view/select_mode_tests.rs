use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::PlainHistoryCell;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

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
        "pager": {"page_down": "c"}
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
    press(&mut view, &cells, KeyCode::Char('c'));

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

#[test]
fn find_accepts_at_the_match_and_cancel_restores_the_saved_selection() {
    let mut cells = vec![cell(&["alpha beta gamma"])];
    for _ in 0..6 {
        cells.push(cell(&["filler"]));
    }
    cells.push(cell(&["distant"]));
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("gamma"));
    while view.advance_search(&cells) {}
    press(&mut view, &cells, KeyCode::Enter);
    let accepted_cursor = view
        .select_mode
        .expect("select mode")
        .cursor(view.selection.as_ref().expect("selection"));
    let accepted_position = view.position;

    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("distant"));
    while view.advance_search(&cells) {}
    render(&mut view, &cells);
    press(&mut view, &cells, KeyCode::Esc);

    assert_eq!(view.position, accepted_position);
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection")),
        accepted_cursor
    );
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Right);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ga"));

    press(&mut view, &cells, KeyCode::Char('/'));
    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 6,
    );
    view.render(narrow, &mut Buffer::empty(narrow), &cells);
    press(&mut view, &cells, KeyCode::Esc);
    view.render(narrow, &mut Buffer::empty(narrow), &cells);

    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(matches!(
        view.handle_key(
            KeyEvent::new(KeyCode::Char('\u{3}'), KeyModifiers::NONE),
            &cells,
        ),
        Some(ViewAction::Changed)
    ));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ga"));

    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("alpha"));
    while view.advance_search(&cells) {}
    press(&mut view, &cells, KeyCode::Enter);
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection"))
            .offset,
        0
    );
    assert_eq!(view.selected_text(&cells).as_deref(), Some("alpha beta g"));

    let key = accepted_cursor.key;
    view.select_mode = None;
    view.selection
        .as_mut()
        .expect("selection")
        .set_keyboard_range(
            Anchor {
                offset: 6,
                ..accepted_cursor
            },
            Anchor {
                key,
                offset: 10,
                ..accepted_cursor
            },
        );
    assert!(view.begin_select_mode(&cells));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta"));

    for (query, expected_offset, expected_text) in [("\u{301}", 0, "a\u{301}"), ("\nbeta", 4, "b")]
    {
        let cells = vec![cell(&["a\u{301}", "beta"])];
        let mut view = TranscriptView::default();
        render(&mut view, &cells);
        assert!(view.begin_select_mode(&cells));
        press(&mut view, &cells, KeyCode::Char('/'));
        assert!(view.paste_search(query));
        while view.advance_search(&cells) {}
        press(&mut view, &cells, KeyCode::Enter);
        assert_eq!(
            view.select_mode
                .expect("select mode")
                .cursor(view.selection.as_ref().expect("selection"))
                .offset,
            expected_offset
        );
        press(&mut view, &cells, KeyCode::Char('v'));
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected_text));
    }
}

#[test]
fn accepted_find_repeats_into_the_select_cursor() {
    let cells = vec![cell(&["needle first needle second"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("needle"));
    while view.advance_search(&cells) {}
    press(&mut view, &cells, KeyCode::Enter);
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection"))
            .offset,
        13
    );

    press(&mut view, &cells, KeyCode::Char('/'));
    press(&mut view, &cells, KeyCode::Enter);
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection"))
            .offset,
        13
    );

    press(&mut view, &cells, KeyCode::Char('n'));
    press(&mut view, &cells, KeyCode::Right);
    while view.advance_search(&cells) {}
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection"))
            .offset,
        14
    );

    press(&mut view, &cells, KeyCode::Char('n'));
    while view.advance_search(&cells) {}
    assert_eq!(
        view.select_mode
            .expect("select mode")
            .cursor(view.selection.as_ref().expect("selection"))
            .offset,
        0
    );

    press(&mut view, &cells, KeyCode::Char('v'));
    assert!(matches!(
        view.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::SHIFT),
            &cells,
        ),
        Some(ViewAction::Changed)
    ));
    while view.advance_search(&cells) {}
    let mode = view.select_mode.expect("select mode");
    assert!(mode.mark.is_some());
    assert_eq!(
        mode.cursor(view.selection.as_ref().expect("selection"))
            .offset,
        13
    );
}

#[test]
fn accepted_find_pins_an_expanded_match_before_select_mode_exits() {
    let output = format!(
        "{}hidden needle\n{}",
        "head\n".repeat(/*n*/ 20),
        "tail\n".repeat(/*n*/ 20)
    );
    let command = crate::exec_cell::ExecCell::new(
        crate::exec_cell::ExecCall {
            call_id: "command".to_string(),
            command: vec!["echo".to_string()],
            parsed: Vec::new(),
            output: Some(crate::exec_cell::CommandOutput::new(
                /*exit_code*/ 0, output,
            )),
            source: codex_app_server_protocol::CommandExecutionSource::Agent,
            start_time: None,
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    let cells = vec![Arc::new(command) as Arc<dyn HistoryCell>];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("needle"));
    while view.advance_search(&cells) {}
    press(&mut view, &cells, KeyCode::Enter);

    let anchor = view.search.match_anchor().expect("accepted hidden match");
    render(&mut view, &cells);
    let cursor = view
        .select_mode
        .expect("select mode")
        .cursor(view.selection.as_ref().expect("selection"));
    let index = view.resolve(&cells, cursor);
    let row = view
        .layout(&cells, index)
        .expect("cursor layout")
        .row_for_offset(cursor.offset);
    assert!(
        view.visible
            .iter()
            .any(|visible| visible.index == index && visible.row == row)
    );
    let expanded = view
        .search
        .match_layout(anchor.key)
        .expect("expanded search layout")
        .text()
        .to_owned();
    press(&mut view, &cells, KeyCode::Char('/'));
    press(&mut view, &cells, KeyCode::Esc);
    assert_eq!(
        view.selection
            .as_ref()
            .expect("selection")
            .snapshot
            .pinned
            .get(&anchor.key)
            .expect("expanded origin pinned")
            .text(),
        expanded
    );
    assert!(matches!(
        view.handle_key(KeyCode::Char('q').into(), &cells),
        Some(ViewAction::ExitSelectMode)
    ));
    assert_eq!(
        view.held_reading
            .as_ref()
            .expect("paused snapshot")
            .pinned
            .get(&anchor.key)
            .expect("expanded match pinned")
            .text(),
        expanded
    );
}
#[test]
fn beginning_motion_finishes_at_the_first_grapheme_after_paging() {
    let mut cells = vec![cell(&["newer"])];
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..Default::default()
    };
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    assert_eq!(view.history, TranscriptHistoryState::LoadingBeginning);

    cells.insert(/*index*/ 0, cell(&["older"]));
    view.history_loaded(&cells, 0..1);
    view.history = TranscriptHistoryState::Complete;
    view.complete_select_beginning(&cells);
    press(&mut view, &cells, KeyCode::Char('v'));
    assert_eq!(view.selected_text(&cells).as_deref(), Some("o"));
}

#[test]
fn shift_click_extends_from_the_keyboard_marks_fixed_anchor() {
    let cells = vec![cell(&["abcdef"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Right);
    press(&mut view, &cells, KeyCode::Right);
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Right);
    press(&mut view, &cells, KeyCode::Right);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("cde"));

    view.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::SHIFT,
        },
        &cells,
    );

    assert_eq!(view.selected_text(&cells).as_deref(), Some("abc"));
}

#[test]
fn existing_mouse_selection_keeps_its_word_or_line_unit() {
    let word_cells = vec![cell(&["alpha beta"])];
    let mut word_view = TranscriptView::default();
    render(&mut word_view, &word_cells);
    word_view.begin_selection(
        &word_cells,
        /*column*/ 1,
        /*row*/ 0,
        /*clicks*/ 2,
    );
    assert!(word_view.begin_select_mode(&word_cells));
    press(&mut word_view, &word_cells, KeyCode::Char('h'));
    assert_eq!(
        word_view.selected_text(&word_cells).as_deref(),
        Some(&"alpha"[..4])
    );

    let line_cells = vec![cell(&["first", "second"])];
    let mut line_view = TranscriptView::default();
    render(&mut line_view, &line_cells);
    line_view.begin_selection(
        &line_cells,
        /*column*/ 1,
        /*row*/ 1,
        /*clicks*/ 3,
    );
    assert!(line_view.begin_select_mode(&line_cells));
    press(&mut line_view, &line_cells, KeyCode::Char('k'));
    assert_eq!(
        line_view.selected_text(&line_cells).as_deref(),
        Some("first\nsecond")
    );
}

#[test]
fn passive_mouse_events_do_not_reanchor_a_reverse_line_mark() {
    let cells = vec![cell(&["first", "second", "third"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Char('j'));
    press(&mut view, &cells, KeyCode::Char('V'));
    press(&mut view, &cells, KeyCode::Char('k'));

    for kind in [MouseEventKind::Moved, MouseEventKind::ScrollDown] {
        view.handle_mouse(
            MouseEvent {
                kind,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
    }
    view.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 80,
            row: 80,
            modifiers: KeyModifiers::NONE,
        },
        &cells,
    );
    press(&mut view, &cells, KeyCode::Char('j'));

    assert_eq!(view.selected_text(&cells).as_deref(), Some("second\n"));
}

#[test]
fn select_mode_renders_cursor_and_scannable_footer_hints() {
    let cells = vec![cell(&["alpha beta", "gamma"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    assert!(view.begin_select_mode(&cells));
    let buffer = render(&mut view, &cells);
    let [unmarked_wide, footer, unmarked_narrow] = [240, 64, 32].map(|width| {
        view.footer_with_navigation(
            width,
            crate::motion::MotionMode::Reduced,
            /*latest_navigation*/ "",
        )
        .expect("select footer")
    });
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 64,
        footer.text.height() as u16,
    );
    let mut footer_buffer = Buffer::empty(area);
    Paragraph::new(footer.text).render(area, &mut footer_buffer);

    press(&mut view, &cells, KeyCode::Char('g'));
    press(&mut view, &cells, KeyCode::Char('v'));
    for _ in 0..5 {
        press(&mut view, &cells, KeyCode::Right);
    }
    let marked_buffer = render(&mut view, &cells);
    let [marked_wide, marked] = [240, 64].map(|width| {
        view.footer_with_navigation(
            width,
            crate::motion::MotionMode::Reduced,
            /*latest_navigation*/ "",
        )
        .expect("select footer")
    });

    press(&mut view, &cells, KeyCode::Esc);
    press(&mut view, &cells, KeyCode::Char('/'));
    assert!(view.paste_search("beta"));
    while view.advance_search(&cells) {}
    let find_buffer = render(&mut view, &cells);
    let find_footer = view
        .footer_with_navigation(
            /*width*/ 64,
            crate::motion::MotionMode::Reduced,
            /*latest_navigation*/ "",
        )
        .expect("find footer");
    let find_footer_text = find_footer
        .text
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    press(&mut view, &cells, KeyCode::Enter);
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Right);
    let find_marked_buffer = render(&mut view, &cells);

    insta::assert_snapshot!(format!(
        "{buffer:?}\nunmarked wide: {}\nunmarked:\n{footer_buffer:?}\nunmarked narrow: {}\nmarked:\n{marked_buffer:?}\nmarked wide: {}\nmarked: {}\nfind:\n{find_buffer:?}\nfind footer: {}\nfind cursor: {:?}\nfind marked:\n{find_marked_buffer:?}",
        unmarked_wide.text,
        unmarked_narrow.text,
        marked_wide.text,
        marked.text,
        find_footer_text,
        find_footer.cursor_column,
    ));
}

fn select_table_at_match(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    query: &str,
) -> TranscriptView {
    let mut view = TranscriptView::default();
    crate::transcript_view::tests::render(&mut view, cells, width, /*height*/ 32);
    assert!(view.begin_select_mode(cells));
    press(&mut view, cells, KeyCode::Char('/'));
    assert!(view.paste_search(query));
    while view.advance_search(cells) {}
    press(&mut view, cells, KeyCode::Enter);
    view
}

// These view tests receive the semantic event produced by the app's chord router.
fn mark_markdown(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) {
    let key = crate::keymap::dispatch_event(crate::keymap::KeymapActionId {
        context: crate::keymap::KeymapContext::Pager,
        action: "mark_markdown",
    })
    .unwrap();
    press(view, cells, key.code);
}

#[test]
fn keyboard_table_mark_selects_cells_and_swaps_rectangle_corners() {
    let cells: [Arc<dyn HistoryCell>; 1] = [Arc::new(AgentMarkdownCell::new(
        "| Left | Right |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |".into(),
        std::path::Path::new("/"),
    ))];
    let mut view = select_table_at_match(&cells, /*width*/ 32, "Beta");
    let footer = |view: &TranscriptView| {
        view.footer_with_navigation(/*width*/ 32, crate::motion::MotionMode::Reduced, "")
            .unwrap()
            .text
    };
    let before = footer(&view);

    mark_markdown(&mut view, &cells);
    press(&mut view, &cells, KeyCode::Down);
    press(&mut view, &cells, KeyCode::Char('h'));
    let rectangle = "|  |  |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |";
    assert!(matches!(
        view.handle_key(KeyCode::Char('y').into(), &cells),
        Some(ViewAction::Copy(text)) if text == rectangle
    ));

    let marked = crate::transcript_view::tests::render(
        &mut view, &cells, /*width*/ 32, /*height*/ 5,
    );
    let highlight: String = marked
        .content()
        .chunks(32)
        .map(|row| {
            row.iter()
                .map(|cell| {
                    use ratatui::style::Modifier;
                    if cell.modifier.contains(Modifier::UNDERLINED) {
                        "@"
                    } else if cell.modifier.contains(Modifier::REVERSED) {
                        cell.symbol()
                    } else {
                        " "
                    }
                })
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(format!("{before}\n{highlight}\n{}", footer(&view)));

    press(&mut view, &cells, KeyCode::Char('/'));
    press(&mut view, &cells, KeyCode::Esc);
    assert_eq!(view.selected_text(&cells).as_deref(), Some(rectangle));
    press(&mut view, &cells, KeyCode::Char('o'));
    press(&mut view, &cells, KeyCode::Down);
    press(&mut view, &cells, KeyCode::Left);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("Gamma"));
    press(&mut view, &cells, KeyCode::Esc);
    assert_eq!(view.selected_text(&cells), None);
}

#[test]
fn keyboard_table_header_rectangle_excludes_record_values() {
    let cells = [Arc::new(AgentMarkdownCell::new(
        "| Left | Right |\n|---|---|\n| alpha | Beta |".into(),
        std::path::Path::new("/"),
    )) as Arc<dyn HistoryCell>];
    for width in [10, 32] {
        let mut view = select_table_at_match(&cells, width, "Left");
        mark_markdown(&mut view, &cells);
        press(&mut view, &cells, KeyCode::Right);
        assert_eq!(
            view.selected_text(&cells).as_deref(),
            Some("| Left | Right |\n|---|---|"),
            "width {width}",
        );
    }
}

#[test]
fn shift_click_extends_from_the_fixed_in_cell_text_anchor() {
    let cells = [Arc::new(AgentMarkdownCell::new(
        "| Text | Other |\n|---|---|\n| abcde | 1 |".into(),
        std::path::Path::new("/"),
    )) as Arc<dyn HistoryCell>];
    for (motion, target, expected) in [(KeyCode::Left, "e", "bcd"), (KeyCode::Right, "a", "ab")] {
        let mut view = select_table_at_match(&cells, /*width*/ 32, "bcd");
        mark_markdown(&mut view, &cells);
        press(&mut view, &cells, KeyCode::Char('v'));
        press(&mut view, &cells, motion);
        let buffer = crate::transcript_view::tests::render(
            &mut view, &cells, /*width*/ 32, /*height*/ 12,
        );
        let index = buffer
            .content()
            .iter()
            .rposition(|cell| cell.symbol() == target)
            .unwrap();
        let (column, row) = buffer.pos_of(index);
        view.handle_selection_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::SHIFT,
            },
            &cells,
            /*patch_approval*/ None,
        );
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
    }
}

#[test]
fn in_cell_word_motion_projects_boundaries_past_escaped_pipes() {
    for (header, query, motion, expected) in [
        (r"A\|界語", "語", 'b', "界語"),
        (r"A\|alpha", "alpha", 'e', "alpha"),
    ] {
        let cells = [Arc::new(AgentMarkdownCell::new(
            format!("| {header} | B |\n|---|---|"),
            std::path::Path::new("/"),
        )) as Arc<dyn HistoryCell>];
        let mut view = select_table_at_match(&cells, /*width*/ 8, query);
        mark_markdown(&mut view, &cells);
        press(&mut view, &cells, KeyCode::Char('v'));
        press(&mut view, &cells, KeyCode::Char(motion));
        assert!(matches!(
            view.handle_key(KeyCode::Char('y').into(), &cells),
            Some(ViewAction::Copy(text)) if text == expected
        ));
    }
}

#[test]
fn shift_click_replaces_the_cell_mark_at_the_same_endpoint() {
    let cells = [Arc::new(AgentMarkdownCell::new(
        "| Left | Right |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |".into(),
        std::path::Path::new("/"),
    )) as Arc<dyn HistoryCell>];
    let mut view = select_table_at_match(&cells, /*width*/ 32, "Beta");
    mark_markdown(&mut view, &cells);
    press(&mut view, &cells, KeyCode::Down);
    crate::transcript_view::tests::render(&mut view, &cells, /*width*/ 32, /*height*/ 12);
    let selection = view.selection.as_ref().unwrap();
    let bounds = (selection.start, selection.end);
    let (row, line) = view
        .visible
        .iter()
        .enumerate()
        .find(|(_, visible)| {
            visible.key == bounds.1.key
                && visible.layout.row_for_offset(bounds.1.offset) == visible.row
        })
        .unwrap();
    let column = line
        .layout
        .column_for_offset_on_row(line.row, bounds.1.offset);
    view.handle_selection_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row: row as u16,
            modifiers: KeyModifiers::SHIFT,
        },
        &cells,
        /*patch_approval*/ None,
    );
    let selection = view.selection.as_ref().unwrap();
    assert_eq!((selection.start, selection.end), bounds);
    assert!(view.table_mark().is_none());
}

#[test]
fn keyboard_cell_word_motion_crosses_a_wrapped_identifier() {
    let cells = [Arc::new(AgentMarkdownCell::new(
        "| Name | Value |\n|---|---|\n| abcdefghijklmnop next | 1 |".into(),
        std::path::Path::new("/"),
    )) as Arc<dyn HistoryCell>];
    let mut view = select_table_at_match(&cells, /*width*/ 12, "abcd");
    mark_markdown(&mut view, &cells);
    press(&mut view, &cells, KeyCode::Char('v'));
    press(&mut view, &cells, KeyCode::Char('e'));
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some("abcdefghijklmnop")
    );
    press(&mut view, &cells, KeyCode::Char('b'));
    press(&mut view, &cells, KeyCode::Char('w'));
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some("abcdefghijklmnop n")
    );
}

#[test]
fn keyboard_table_motion_skips_record_labels_and_reaches_empty_cells() {
    let cells: [Arc<dyn HistoryCell>; 1] = [Arc::new(AgentMarkdownCell::new(
        "| Left | Right |\n|---|---|\n| alpha | Beta |\n| Gamma |  |\n| omega | delta |".into(),
        std::path::Path::new("/"),
    ))];
    for width in [32, 10] {
        let mut view = select_table_at_match(&cells, width, "Beta");
        mark_markdown(&mut view, &cells);
        press(&mut view, &cells, KeyCode::Char('j'));
        press(&mut view, &cells, KeyCode::Char('j'));
        assert_eq!(
            view.selected_text(&cells).as_deref(),
            Some("|  |  |\n|---|---|\n|  | Beta |\n|  |  |\n|  | delta |"),
            "width {width}"
        );
        press(&mut view, &cells, KeyCode::Char('k'));
        press(&mut view, &cells, KeyCode::Esc);
        mark_markdown(&mut view, &cells);
        assert_eq!(view.selected_text(&cells), None);
        press(&mut view, &cells, KeyCode::Char('k'));
        assert!(
            view.selected_text(&cells)
                .is_some_and(|text| text.contains("Beta"))
        );
        if width == 32 {
            press(&mut view, &cells, KeyCode::Down);
            press(&mut view, &cells, KeyCode::Esc);
            press(&mut view, &cells, KeyCode::Up); // Cross the separator.
            press(&mut view, &cells, KeyCode::Up);
            mark_markdown(&mut view, &cells);
            assert_eq!(view.selected_text(&cells).as_deref(), Some("Beta"));
        }
    }
}

#[test]
fn keyboard_cell_text_crosses_wraps_without_selecting_neighbors() {
    let cells: [Arc<dyn HistoryCell>; 1] = [Arc::new(AgentMarkdownCell::new(
        "| Description | Size |\n|---|---|\n| alpha **界 beta gamma delta** omega | 123 |\n| following | 456 |".into(),
        std::path::Path::new("/"),
    ))];
    for width in [34, 12] {
        let mut view = select_table_at_match(&cells, width, "界");
        mark_markdown(&mut view, &cells);
        press(&mut view, &cells, KeyCode::Char('v'));
        assert_eq!(view.selected_text(&cells).as_deref(), Some("界"));
        press(&mut view, &cells, KeyCode::Down);
        assert!(
            view.selected_text(&cells).unwrap().starts_with("界 beta"),
            "width {width}"
        );
        press(&mut view, &cells, KeyCode::Up);
        press(&mut view, &cells, KeyCode::Char('w'));
        assert_eq!(view.selected_text(&cells).as_deref(), Some("界 b"));
        press(&mut view, &cells, KeyCode::End);
        press(&mut view, &cells, KeyCode::Right);
        let text = "界 beta gamma delta omega";
        assert!(matches!(
            view.handle_key(KeyCode::Char('y').into(), &cells),
            Some(ViewAction::Copy(copy)) if copy == text
        ));
        view.copy_selected_text_with(
            &cells,
            text,
            /*clear_selection*/ false,
            |text, format| {
                assert_eq!(
                    (text, format),
                    (
                        "界 beta gamma delta omega",
                        crate::clipboard_copy::CopyFormat::MarkdownSelection(
                            "**界 beta gamma delta** omega".into()
                        ),
                    )
                );
                Ok(CopyStatus::Confirmed)
            },
        )
        .unwrap();

        press(&mut view, &cells, KeyCode::Char('o'));
        press(&mut view, &cells, KeyCode::Home);
        press(&mut view, &cells, KeyCode::Left);
        assert_eq!(
            view.selected_text(&cells).as_deref(),
            Some("alpha 界 beta gamma delta omega"),
            "width {width}"
        );
    }
}
