//! Mouse edits preserve buffered typing and keep placeholder payloads in sync.

use super::super::tests::new_test_composer;
use super::*;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseButton::Right;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Up;
use pretty_assertions::assert_eq;

fn mouse(composer: &mut ChatComposer, area: Rect, kind: MouseEventKind, column: u16, row: u16) {
    let event = MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    assert!(composer.prepare_mouse(event));
    composer.render(area, &mut Buffer::empty(area));
    assert!(composer.handle_mouse(event));
}

#[test]
fn composer_mouse_edits_flush_typing_and_reconcile_placeholders() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("hello");
    composer
        .draft
        .paste_burst
        .begin_with_retro_grabbed("\nworld".into(), Instant::now());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    mouse(&mut composer, area, Down(Left), x - 5, y);
    composer.insert_str("X");
    assert_eq!(composer.current_text(), "hello\nXworld");
    composer.handle_paste("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    for path in [Some("first.png"), Some("second.png"), None] {
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        // Double-clicking inside the label selects the entire atomic element.
        for kind in [Down(Left), Up(Left), Down(Left), Up(Left)] {
            mouse(&mut composer, area, kind, x - 1, y);
        }
        if let Some(path) = path {
            let path = PathBuf::from(path);
            composer.attach_image(path.clone());
            assert_eq!(composer.attachments.local_image_paths(), vec![path]);
        } else {
            composer.handle_key_event(KeyCode::Char('é').into());
            assert!(composer.attachments.is_empty());
        }
        assert!(composer.draft.pending_pastes.is_empty());
    }
}

#[test]
fn recovery_keeps_unselected_text_after_a_held_replace_key() {
    for (text, expected) in [("abcd", "aXd"), ("!abcd", "!aXd")] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_text_content(text.into(), Vec::new(), Vec::new());
        composer.draft.textarea.set_cursor(/*pos*/ 0);
        composer.set_vim_enabled(/*enabled*/ true);
        composer.handle_key_event(KeyCode::Char('R').into());
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        );
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        for (kind, column) in [
            (Down(Left), x + 1),
            (MouseEventKind::Drag(Left), x + 3),
            (Up(Left), x + 3),
        ] {
            mouse(&mut composer, area, kind, column, y);
        }
        let now = Instant::now();
        composer.handle_input_basic_with_time(KeyCode::Char('X').into(), now);
        let before = composer.draft_snapshot();
        assert_eq!(composer.recovery_snapshot().text, expected);
        assert_eq!(composer.draft_snapshot(), before);
        assert!(composer.handle_paste_burst_flush(now + PasteBurst::recommended_flush_delay()));
        assert_eq!(composer.current_text(), expected);
    }
}

#[test]
fn backspace_deletes_a_body_selection_before_an_absorbed_prefix() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("!review text");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    for (kind, column) in [
        (Down(Left), x),
        (MouseEventKind::Drag(Left), x - "review text".len() as u16),
        (Up(Left), x - "review text".len() as u16),
    ] {
        mouse(&mut composer, area, kind, column, y);
    }

    composer.handle_key_event(KeyCode::Backspace.into());

    assert_eq!(composer.current_text(), "!");
    assert_eq!(composer.draft.textarea.mouse_selection_range(), None);
}

#[test]
fn selected_text_navigation_does_not_recall_history_or_select_images() {
    for (key, remote) in [
        (KeyCode::Up, false),
        (KeyCode::Down, false),
        (KeyCode::Up, true),
    ] {
        let (mut composer, _rx) = new_test_composer();
        for text in ["first", "second"] {
            composer
                .history
                .record_local_submission(HistoryEntry::new(text.into()));
        }
        composer.handle_key_event(KeyCode::Up.into());
        if remote {
            composer.set_remote_image_urls(vec!["https://example.com/image.png".into()]);
        }
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
        );
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        if remote {
            composer.draft.textarea.set_cursor(/*pos*/ 0);
            composer.handle_key_event(KeyCode::Up.into());
        }
        let (start, end) = if key == KeyCode::Up {
            (x, x - 6)
        } else {
            (x - 6, x)
        };
        for (kind, column) in [
            (Down(Left), start),
            (MouseEventKind::Drag(Left), end),
            (Up(Left), end),
        ] {
            mouse(&mut composer, area, kind, column, y);
        }
        assert_eq!(composer.attachments.selected_remote_image_index, None);
        composer.handle_key_event(key.into());
        assert_eq!(
            (
                composer.current_text(),
                composer.draft.textarea.mouse_selection_range(),
                composer.attachments.selected_remote_image_index
            ),
            ("second".to_string(), None, None)
        );
    }
}

#[test]
fn right_click_copy_uses_the_refreshed_editor_bounds() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("hello world");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    for (kind, column) in [
        (Down(Left), x - 11),
        (MouseEventKind::Drag(Left), x - 6),
        (Up(Left), x - 6),
    ] {
        mouse(&mut composer, area, kind, column, y);
    }
    let click = MouseEvent {
        kind: Down(Right),
        column: x - 9,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        composer.copy_selection(&TuiEvent::Mouse(click), |text| {
            assert_eq!(text, "hello");
            Ok(CopyStatus::Unconfirmed)
        }),
        Some((5, Ok(CopyStatus::Unconfirmed)))
    );

    let shifted_area = Rect::new(
        /*x*/ 0, /*y*/ 10, /*width*/ 20, /*height*/ 4,
    );
    assert!(composer.prepare_mouse(click));
    composer.render(shifted_area, &mut Buffer::empty(shifted_area));
    assert_eq!(
        composer.copy_selection(&TuiEvent::Mouse(click), |_| unreachable!()),
        None
    );
    let (_, row) = composer.cursor_pos(shifted_area).unwrap();
    assert_eq!(
        composer.copy_selection(&TuiEvent::Mouse(MouseEvent { row, ..click }), |text| {
            assert_eq!(text, "hello");
            Ok(CopyStatus::Unconfirmed)
        }),
        Some((5, Ok(CopyStatus::Unconfirmed)))
    );
    assert_eq!(composer.current_text(), "hello world");
    let event = TuiEvent::Mouse(MouseEvent { row, ..click });
    composer.copy_selection(&event, |_| Ok(CopyStatus::Pending(1)));
    // A fresh pointer selection must not be cleared by the older copy.
    for (kind, column) in [
        (Down(Left), x - 11),
        (MouseEventKind::Drag(Left), x - 6),
        (Up(Left), x - 6),
    ] {
        mouse(&mut composer, shifted_area, kind, column, row);
    }
    let selection = composer.draft.textarea.mouse_selection_range();
    assert_eq!(
        composer.finish_copy(&(1, Ok(CopyStatus::Confirmed)), /*current*/ true),
        None
    );
    assert_eq!(composer.draft.textarea.mouse_selection_range(), selection);
    let key = TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    composer.copy_selection(&key, |_| Ok(CopyStatus::Pending(2)));
    let paste_target = composer.clipboard_paste_target(Some(KeyCode::Insert));
    assert_eq!(
        composer.finish_copy(&(2, Ok(CopyStatus::Confirmed)), /*current*/ true),
        Some(5)
    );
    assert_eq!(composer.draft.textarea.mouse_selection_range(), None);
    assert_ne!(
        composer.clipboard_paste_target(Some(KeyCode::Insert)),
        paste_target
    );
}

#[test]
fn selection_release_copies_and_keeps_the_editable_selection() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("hello world");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    let release = MouseEvent {
        kind: Up(Left),
        column: x - 6,
        row: y,
        modifiers: KeyModifiers::NONE,
    };
    for (kind, column) in [
        (Down(Left), x - 11),
        (MouseEventKind::Drag(Left), x - 6),
        (Up(Left), x - 6),
    ] {
        mouse(&mut composer, area, kind, column, y);
    }

    let selection_copy = crate::tui::SelectionCopy::on_select(
        /*clipboard*/ true, /*primary*/ true, /*clear_selection*/ false,
    );
    let owner = std::sync::Arc::new(());
    let retained = std::sync::Arc::downgrade(&owner);
    assert_eq!(
        composer.copy_selection_on_release(release, selection_copy, |text, policy| {
            assert_eq!((text, policy), ("hello", selection_copy));
            (Ok(CopyStatus::Confirmed), Some(owner))
        }),
        Some((5, Some(Ok(CopyStatus::Confirmed))))
    );

    let mut buffer = Buffer::empty(area);
    composer.render(area, &mut buffer);
    let highlight = (x - 11..x)
        .map(|column| {
            buffer[(column, y)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        highlight,
        vec![
            true, true, true, true, true, false, false, false, false, false, false
        ]
    );
    assert!(retained.upgrade().is_some());

    composer.insert_str("X");
    assert_eq!(composer.current_text(), "X world");
    assert!(retained.upgrade().is_none());
}

#[test]
fn selection_release_reports_only_clipboard_result() {
    for (clipboard, expected) in [(true, Some(Ok(CopyStatus::Pending(7)))), (false, None)] {
        let (mut composer, _rx) = new_test_composer();
        composer.insert_str("hello world");
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
        );
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        let release = MouseEvent {
            kind: Up(Left),
            column: x - 6,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        for (kind, column) in [
            (Down(Left), x - 11),
            (MouseEventKind::Drag(Left), x - 6),
            (Up(Left), x - 6),
        ] {
            mouse(&mut composer, area, kind, column, y);
        }
        let selection = composer.draft.textarea.mouse_selection_range();
        let policy = crate::tui::SelectionCopy::on_select(
            clipboard, /*primary*/ true, /*clear_selection*/ false,
        );
        assert_eq!(
            composer.copy_selection_on_release(release, policy, |text, actual| {
                assert_eq!((text, actual), ("hello", policy));
                (Ok(CopyStatus::Pending(7)), Some(std::sync::Arc::new(())))
            }),
            Some((5, expected))
        );
        composer.finish_copy(&(7, Ok(CopyStatus::Confirmed)), /*current*/ true);
        assert_eq!(composer.draft.textarea.mouse_selection_range(), selection);
    }
}
